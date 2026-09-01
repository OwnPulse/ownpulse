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
use crate::integrations::google_calendar::{DayAggregate, FetchError, GoogleCalendarClient};
use crate::jobs::{SyncError, try_with_user_sync_lock};

/// Interval between sync runs (15 minutes) — same cadence as Garmin/Oura.
const SYNC_INTERVAL_SECS: u64 = 900;

/// Minimum time between sync *attempts* for a single user — see
/// `oura_sync::MIN_SYNC_INTERVAL_SECS` for the rationale.
const MIN_SYNC_INTERVAL_SECS: i64 = 60;

/// Source key used for both the `integration_tokens.source` column and the
/// per-user advisory sync lock.
const SOURCE: &str = "google_calendar";

/// How many days back of the *rolling* recompute window extends, always,
/// regardless of `last_synced_at`. `last_synced_at` is an instant, not a day
/// boundary — using it as the fetch window's start would mean a sync soon
/// after a previous one only re-fetches the last few minutes, so a day
/// that's already in the past (and thus never revisited by a
/// last-synced-at-anchored window) can never be corrected if a meeting on it
/// is later cancelled or rescheduled upstream. Recomputing the full rolling
/// window on every sync — and writing every day in it, including days with
/// zero events (see `sync_user`) — is what makes `calendar_days` converge to
/// the true current state of the calendar rather than a stale snapshot.
const ROLLING_WINDOW_DAYS: i64 = 7;

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

    // Only used to enumerate which users are connected — `sync_user` below
    // re-reads each user's token row itself, from inside that user's
    // advisory lock, so a concurrent refresh elsewhere can't be clobbered by
    // a stale read taken out here.
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
                user_id,
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

    // Only used for the connected/not-connected check and the cooldown
    // check below — `sync_user` re-reads the token row itself once the
    // advisory lock is held (see `run_sync`'s comment on the same pattern).
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
            user_id,
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

/// Refresh the access token, persist the result, and return the new access
/// token. Falls back to the existing stored refresh token when Google's
/// response omits one (it doesn't reissue a refresh token on every refresh).
async fn refresh_and_store(
    pool: &PgPool,
    http_client: &reqwest::Client,
    config: &Config,
    user_id: Uuid,
    refresh_token: &str,
    existing_refresh_token: Option<&str>,
    encryption_key: &[u8; 32],
) -> Result<String, String> {
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

    let refresh_to_store = new_tokens
        .refresh_token
        .as_deref()
        .or(existing_refresh_token);

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

    Ok(new_tokens.access_token)
}

/// Sync data for a single user. Returns the number of `calendar_days` rows
/// written on success. `last_synced_at` is only advanced when the fetch
/// succeeds — a failure returns `Err` so the caller records
/// `last_sync_error` and leaves the watermark where it was, so the failed
/// window is retried on the next run rather than silently skipped.
///
/// Re-reads the current token row itself (rather than accepting one from the
/// caller) — this function only ever runs while the caller holds this user's
/// advisory sync lock, so reading here (instead of before the lock was
/// acquired) means a concurrent refresh elsewhere can't be raced and
/// clobbered by a stale read.
async fn sync_user(
    pool: &PgPool,
    client: &GoogleCalendarClient,
    config: &Config,
    http_client: &reqwest::Client,
    user_id: Uuid,
    encryption_key: &[u8; 32],
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<u32, String> {
    // Re-derive the previous-key fallback locally rather than threading it
    // through from the caller — keeps this function's signature (and the
    // `try_with_user_sync_lock` closures that call it) shorter, at the cost
    // of one redundant hex-parse per sync.
    let prev_key = config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()
        .map_err(|e| format!("bad previous encryption key: {e}"))?;

    let token_row =
        integration_tokens::list_for_user(pool, user_id, encryption_key, prev_key.as_ref())
            .await
            .map_err(|e| format!("failed to reload Google Calendar token: {e}"))?
            .into_iter()
            .find(|t| t.source == SOURCE)
            .ok_or("Google Calendar is no longer connected for this user")?;

    let mut access_token = token_row.access_token.clone();

    // Refresh proactively if the token is expired, unknown-lifetime (no
    // `expires_at` — treat as due for refresh rather than never refreshing),
    // or within 60s of expiring, to avoid a request racing expiry mid-flight.
    let needs_refresh = match token_row.expires_at {
        None => true,
        Some(expires_at) => expires_at < Utc::now() + chrono::Duration::seconds(60),
    };
    if needs_refresh {
        let refresh_token = token_row
            .refresh_token
            .as_deref()
            .ok_or("Google Calendar token expired and no refresh token available")?;
        access_token = refresh_and_store(
            pool,
            http_client,
            config,
            user_id,
            refresh_token,
            token_row.refresh_token.as_deref(),
            encryption_key,
        )
        .await?;
    }

    // Always recompute a rolling window anchored on "now" — never on
    // `last_synced_at` — floored to a day boundary. See `ROLLING_WINDOW_DAYS`.
    let now = Utc::now();
    let window_start_date = (now - chrono::Duration::days(ROLLING_WINDOW_DAYS)).date_naive();
    let time_min = window_start_date
        .and_hms_opt(0, 0, 0)
        .map(|ndt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
        .unwrap_or(now);
    let time_max = now + chrono::Duration::days(LOOKAHEAD_DAYS);

    let fetch_result = client
        .fetch_and_aggregate(&access_token, time_min, time_max)
        .await;

    let aggregates = match fetch_result {
        Ok(map) => map,
        Err(FetchError::Unauthorized) => {
            // The access token was rejected outright — refresh (even if we
            // didn't think it was due) and retry exactly once. Covers a
            // token revoked/expired out of band from what `expires_at` says.
            let refresh_token = token_row.refresh_token.as_deref().ok_or(
                "Google Calendar returned 401 and no refresh token is available to recover",
            )?;
            access_token = refresh_and_store(
                pool,
                http_client,
                config,
                user_id,
                refresh_token,
                token_row.refresh_token.as_deref(),
                encryption_key,
            )
            .await?;
            client
                .fetch_and_aggregate(&access_token, time_min, time_max)
                .await
                .map_err(|e| format!("calendar events fetch failed after refresh retry: {e}"))?
        }
        Err(FetchError::Other(msg)) => return Err(format!("calendar events fetch failed: {msg}")),
    };

    // Write every day in the window — including days with zero events —
    // so a day whose meetings were all cancelled/removed upstream is
    // corrected to zero rather than left at a stale nonzero count from a
    // previous sync. `calendar_days::upsert` always overwrites, not adds.
    let mut days_written = 0u32;
    let mut date = window_start_date;
    let window_end_date = time_max.date_naive();
    while date <= window_end_date {
        let aggregate = aggregates.get(&date).copied().unwrap_or(DayAggregate {
            meeting_count: 0,
            meeting_minutes: 0,
        });
        calendar_days::upsert(
            pool,
            user_id,
            date,
            aggregate.meeting_count,
            aggregate.meeting_minutes,
        )
        .await
        .map_err(|e| format!("failed to upsert calendar_days row: {e}"))?;
        days_written += 1;
        date = date
            .succ_opt()
            .ok_or("date overflow while writing calendar_days window")?;
    }

    // Only notify listeners if at least one meeting actually exists in the
    // window — otherwise every 15-minute sync would fire a "data changed"
    // event even when nothing changed.
    if aggregates.values().any(|a| a.meeting_count > 0) {
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

    tracing::info!(user_id = %user_id, days_written, "Google Calendar sync completed");

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

    /// Pure unit coverage for the rolling-window day count, independent of
    /// any HTTP/DB behavior — the window always spans
    /// `ROLLING_WINDOW_DAYS` days back through `LOOKAHEAD_DAYS` days ahead,
    /// inclusive of both endpoints.
    #[test]
    fn rolling_window_spans_expected_number_of_days() {
        let now = Utc::now();
        let window_start_date = (now - chrono::Duration::days(ROLLING_WINDOW_DAYS)).date_naive();
        let window_end_date = (now + chrono::Duration::days(LOOKAHEAD_DAYS)).date_naive();

        let mut count = 0u32;
        let mut date = window_start_date;
        while date <= window_end_date {
            count += 1;
            date = date.succ_opt().unwrap();
        }

        assert_eq!(count as i64, ROLLING_WINDOW_DAYS + LOOKAHEAD_DAYS + 1);
    }
}
