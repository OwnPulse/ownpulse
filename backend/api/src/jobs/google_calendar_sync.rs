// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync job for Google Calendar meeting aggregates.
//!
//! Periodically fetches calendar events for each connected user and writes
//! per-day meeting counts/minutes into `calendar_days` — never event
//! titles, attendees, or descriptions (see `integrations::google_calendar`
//! module docs for the privacy boundary).

use std::time::Duration as StdDuration;

use chrono::Utc;
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{calendar_days, integration_tokens};
use crate::integrations::google;
use crate::integrations::google_calendar::{GoogleCalendarClient, aggregate_by_day};
use crate::jobs::{SyncError, try_with_user_sync_lock};

/// Interval between sync runs (15 minutes) — same cadence as Garmin/Oura.
const SYNC_INTERVAL_SECS: u64 = 900;

/// Minimum time between sync *attempts* for a single user — see
/// `oura_sync::MIN_SYNC_INTERVAL_SECS` for the rationale.
const MIN_SYNC_INTERVAL_SECS: i64 = 60;

/// Source key used for both the `integration_tokens.source` column and the
/// per-user advisory sync lock.
const SOURCE: &str = "google_calendar";

/// How far back to look on the very first sync for a newly-connected
/// account (no watermark yet).
const INITIAL_LOOKBACK_DAYS: i64 = 7;

/// How far *forward* of "now" to fetch on every sync — captures same-day and
/// near-term meetings so the aggregate for "today" is populated before the
/// day ends, not only in retrospect.
const LOOKAHEAD_DAYS: i64 = 1;

/// Spawn the Google Calendar sync background job. Returns the task handle so
/// callers (and tests) can observe shutdown; `main.rs` does not need to await
/// it.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Google Calendar sync job started");

        let mut interval = tokio::time::interval(StdDuration::from_secs(SYNC_INTERVAL_SECS));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await; // consume the immediate first tick

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Google Calendar sync job shutting down");
                    break;
                }
                _ = interval.tick() => {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            tracing::info!("Google Calendar sync job cancelled mid-cycle");
                            break;
                        }
                        result = run_sync(&pool, &config, &http_client, &event_tx, &cancel) => {
                            if let Err(e) = result {
                                tracing::error!(error = %e, "Google Calendar sync run failed");
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Run a single sync cycle for all users with a connected Google Calendar
/// token. `pub` so integration tests can race it against cancellation
/// directly, the same way `spawn`'s loop does.
pub async fn run_sync(
    pool: &PgPool,
    config: &Config,
    http_client: &reqwest::Client,
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
    cancel: &CancellationToken,
) -> Result<(), String> {
    let encryption_key = crypto::parse_encryption_key(&config.encryption_key)
        .map_err(|e| format!("bad encryption key: {e}"))?;
    let prev_key = config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()
        .map_err(|e| format!("bad previous encryption key: {e}"))?;

    // Google Calendar reuses the login GOOGLE_CLIENT_ID/SECRET — if those
    // aren't configured, this integration is unavailable, same as Garmin/Oura
    // being unconfigured.
    if config.google_client_id.is_none() || config.google_client_secret.is_none() {
        return Ok(());
    }

    let client = GoogleCalendarClient::new(
        http_client.clone(),
        config.google_calendar_api_base_url.clone(),
    );

    let tokens = integration_tokens::list_for_user_by_source(
        pool,
        SOURCE,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list Google Calendar tokens: {e}"))?;

    for token_row in tokens {
        if cancel.is_cancelled() {
            tracing::info!("Google Calendar sync pass interrupted by shutdown");
            break;
        }

        let user_id = token_row.user_id;

        let lock_result = try_with_user_sync_lock(pool, SOURCE, user_id, || {
            sync_user(
                pool,
                &client,
                config,
                http_client,
                &token_row,
                &encryption_key,
                event_tx,
            )
        })
        .await;

        match lock_result {
            Ok(Some(Err(e))) => {
                tracing::error!(user_id = %user_id, error = %e, "Google Calendar sync failed for user");
                let _ = integration_tokens::update_sync_error(pool, user_id, SOURCE, &e).await;
            }
            Ok(Some(Ok(_))) => {}
            Ok(None) => {
                tracing::debug!(user_id = %user_id, "skipping Google Calendar sync — already in progress elsewhere");
            }
            Err(e) => {
                tracing::error!(user_id = %user_id, error = %e, "failed to acquire Google Calendar sync lock");
            }
        }
    }

    Ok(())
}

/// On-demand sync for a single user — used by the
/// `POST /integrations/google-calendar/sync` endpoint. Returns the number of
/// `calendar_days` rows written (updated or inserted).
pub async fn sync_user_now(
    pool: &PgPool,
    config: &Config,
    http_client: &reqwest::Client,
    user_id: Uuid,
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<u32, SyncError> {
    let encryption_key = crypto::parse_encryption_key(&config.encryption_key)
        .map_err(|e| SyncError::Upstream(format!("bad encryption key: {e}")))?;
    let prev_key = config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()
        .map_err(|e| SyncError::Upstream(format!("bad previous encryption key: {e}")))?;

    if config.google_client_id.is_none() || config.google_client_secret.is_none() {
        return Err(SyncError::NotConfigured);
    }

    let client = GoogleCalendarClient::new(
        http_client.clone(),
        config.google_calendar_api_base_url.clone(),
    );

    let token_row =
        integration_tokens::list_for_user(pool, user_id, &encryption_key, prev_key.as_ref())
            .await
            .map_err(|e| SyncError::Upstream(format!("failed to load integration tokens: {e}")))?
            .into_iter()
            .find(|t| t.source == SOURCE)
            .ok_or(SyncError::NotConnected)?;

    let has_prior_attempt =
        token_row.last_synced_at.is_some() || token_row.last_sync_error.is_some();
    if has_prior_attempt {
        let elapsed_secs = Utc::now()
            .signed_duration_since(token_row.updated_at)
            .num_seconds();
        if elapsed_secs < MIN_SYNC_INTERVAL_SECS {
            return Err(SyncError::RateLimited {
                retry_after_secs: (MIN_SYNC_INTERVAL_SECS - elapsed_secs).max(1) as u64,
            });
        }
    }

    let lock_result = try_with_user_sync_lock(pool, SOURCE, user_id, || {
        sync_user(
            pool,
            &client,
            config,
            http_client,
            &token_row,
            &encryption_key,
            event_tx,
        )
    })
    .await
    .map_err(|e| SyncError::Upstream(format!("failed to acquire sync lock: {e}")))?;

    let outcome = match lock_result {
        Some(outcome) => outcome,
        None => {
            return Err(SyncError::RateLimited {
                retry_after_secs: 5,
            });
        }
    };

    if let Err(ref e) = outcome {
        let _ = integration_tokens::update_sync_error(pool, user_id, SOURCE, e).await;
    }
    outcome.map_err(SyncError::Upstream)
}

/// Sync data for a single user. Returns the number of `calendar_days` rows
/// written on success. `last_synced_at` is only advanced when the fetch
/// succeeds — a failure returns `Err` so the caller records
/// `last_sync_error` and leaves the watermark where it was, so the failed
/// window is retried on the next run rather than silently skipped.
async fn sync_user(
    pool: &PgPool,
    client: &GoogleCalendarClient,
    config: &Config,
    http_client: &reqwest::Client,
    token_row: &integration_tokens::IntegrationTokenRow,
    encryption_key: &[u8; 32],
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<u32, String> {
    let user_id = token_row.user_id;

    let mut access_token = token_row.access_token.clone();

    if let Some(expires_at) = token_row.expires_at
        && expires_at < Utc::now()
    {
        let refresh_token = token_row
            .refresh_token
            .as_deref()
            .ok_or("Google Calendar token expired and no refresh token available")?;
        let client_id = config
            .google_client_id
            .as_deref()
            .ok_or("GOOGLE_CLIENT_ID not configured")?;
        let client_secret = config
            .google_client_secret
            .as_deref()
            .ok_or("GOOGLE_CLIENT_SECRET not configured")?;

        let new_tokens = google::refresh_access_token(
            http_client,
            client_id,
            client_secret,
            refresh_token,
            &config.google_token_url,
        )
        .await
        .map_err(|e| format!("Google Calendar token refresh failed: {e}"))?;

        let new_expires_at = new_tokens
            .expires_in
            .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

        // Google may not return a new refresh_token on every refresh — keep
        // the existing one if the response omits it.
        let refresh_to_store = new_tokens
            .refresh_token
            .as_deref()
            .or(token_row.refresh_token.as_deref());

        integration_tokens::upsert(
            pool,
            user_id,
            SOURCE,
            &new_tokens.access_token,
            refresh_to_store,
            new_expires_at,
            encryption_key,
        )
        .await
        .map_err(|e| format!("failed to update Google Calendar tokens after refresh: {e}"))?;

        access_token = new_tokens.access_token;
    }

    let time_min = token_row
        .last_synced_at
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(INITIAL_LOOKBACK_DAYS));
    let time_max = Utc::now() + chrono::Duration::days(LOOKAHEAD_DAYS);

    let events = client
        .fetch_events(&access_token, time_min, time_max)
        .await
        .map_err(|e| format!("calendar events fetch failed: {e}"))?;

    let days = aggregate_by_day(&events);
    let mut days_written = 0u32;
    for (date, aggregate) in &days {
        calendar_days::upsert(
            pool,
            user_id,
            *date,
            aggregate.meeting_count,
            aggregate.meeting_minutes,
        )
        .await
        .map_err(|e| format!("failed to upsert calendar_days row: {e}"))?;
        days_written += 1;
    }

    if days_written > 0 {
        let _ = event_tx.send((
            user_id,
            crate::models::explore::DataChangedEvent {
                source: "google_calendar".to_string(),
                record_type: None,
            },
        ));
    }

    // Only advance the watermark once the fetch (and every upsert) succeeded
    // — a failure above returns before this point.
    integration_tokens::update_last_synced(pool, user_id, SOURCE)
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    tracing::info!(user_id = %user_id, days = days_written, "Google Calendar sync completed");

    Ok(days_written)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test for the `main.rs` wiring: `spawn` must respect an
    /// already-cancelled token and return promptly rather than running the
    /// (900s-interval) sync loop. Uses a lazy pool — cancellation is checked
    /// before any query would run, so no real database is needed.
    #[tokio::test]
    async fn spawn_shuts_down_promptly_on_cancellation() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost/db")
            .expect("lazy pool construction should not touch the network");
        let (event_tx, _) = tokio::sync::broadcast::channel(1);
        let cancel = CancellationToken::new();
        cancel.cancel();

        let handle = spawn(
            pool,
            crate::config::test_helpers::minimal_config(),
            reqwest::Client::new(),
            cancel,
            event_tx,
        );

        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("job should shut down promptly once cancelled")
            .expect("job task should not panic");
    }
}
