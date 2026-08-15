// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync job for Oura Ring API data.
//!
//! Periodically fetches readiness, sleep, activity, and heart rate data
//! from Oura for all users with connected Oura integrations.

use std::time::Duration as StdDuration;

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{health_records, integration_tokens, observations};
use crate::integrations::oura::OuraClient;
use crate::jobs::{SyncError, try_with_user_sync_lock};
use crate::models::health_record::CreateHealthRecord;
use crate::models::observation::CreateObservation;

/// Interval between sync runs (15 minutes).
const SYNC_INTERVAL_SECS: u64 = 900;

/// Minimum time between sync *attempts* (manual or scheduled, success or
/// failure) for a single user — see [`sync_user_now`].
const MIN_SYNC_INTERVAL_SECS: i64 = 60;

/// Source key used for both the `integration_tokens.source` column and the
/// per-user advisory sync lock.
const SOURCE: &str = "oura";

/// Spawn the Oura sync background job. Returns the task handle so callers
/// (and tests) can observe shutdown; `main.rs` does not need to await it.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Oura sync job started");

        let mut interval = tokio::time::interval(StdDuration::from_secs(SYNC_INTERVAL_SECS));
        // `Burst` (the default) fires repeated back-to-back ticks to catch up
        // after a missed tick (e.g. a slow previous cycle) — `Delay` just
        // resumes the normal cadence, which is what we want for a periodic
        // poll rather than an at-least-N-times job.
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await; // consume the immediate first tick — first real sync is one interval later

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Oura sync job shutting down");
                    break;
                }
                _ = interval.tick() => {
                    // Race the sync pass itself against cancellation so a slow
                    // provider (or many connected users) can't delay shutdown
                    // past the in-flight HTTP request's timeout — dropping the
                    // future here aborts whatever request/query was pending.
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            tracing::info!("Oura sync job cancelled mid-cycle");
                            break;
                        }
                        result = run_sync(&pool, &config, &http_client, &event_tx, &cancel) => {
                            if let Err(e) = result {
                                tracing::error!(error = %e, "Oura sync run failed");
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Run a single sync cycle for all users with Oura integration tokens.
/// Checks `cancel` between users so a shutdown signal can interrupt a pass
/// partway through rather than waiting for every remaining user. `pub` (not
/// used outside this crate) so integration tests can race it against
/// cancellation directly, the same way [`spawn`]'s loop does, without
/// waiting through the real 15-minute interval.
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

    let client_id = match config.oura_client_id.as_deref() {
        Some(id) => id,
        None => return Ok(()), // Oura not configured, skip
    };
    let client_secret = match config.oura_client_secret.as_deref() {
        Some(s) => s,
        None => return Ok(()),
    };

    let client = OuraClient::new(
        client_id.to_string(),
        client_secret.to_string(),
        config.oura_api_base_url.clone(),
        config.oura_auth_base_url.clone(),
        http_client.clone(),
    );

    let tokens = integration_tokens::list_for_user_by_source(
        pool,
        SOURCE,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list Oura tokens: {e}"))?;

    for token_row in tokens {
        if cancel.is_cancelled() {
            tracing::info!("Oura sync pass interrupted by shutdown");
            break;
        }

        let user_id = token_row.user_id;

        // Advisory lock keyed on (source, user_id): skip (don't block) if
        // another sync for this user is already running — a concurrent
        // manual sync, or (with `replicaCount: 2`) another replica's
        // periodic job iterating the same rows.
        let lock_result = try_with_user_sync_lock(pool, SOURCE, user_id, || {
            sync_user(pool, &client, &token_row, &encryption_key, event_tx)
        })
        .await;

        match lock_result {
            Ok(Some(Err(e))) => {
                tracing::error!(user_id = %user_id, error = %e, "Oura sync failed for user");
                let _ = integration_tokens::update_sync_error(pool, user_id, SOURCE, &e).await;
            }
            Ok(Some(Ok(_))) => {}
            Ok(None) => {
                tracing::debug!(user_id = %user_id, "skipping Oura sync — already in progress elsewhere");
            }
            Err(e) => {
                tracing::error!(user_id = %user_id, error = %e, "failed to acquire Oura sync lock");
            }
        }
    }

    Ok(())
}

/// On-demand sync for a single user — used by the `POST /integrations/oura/sync`
/// endpoint. Looks up the user's Oura connection, runs one sync cycle, and
/// records the outcome the same way the periodic job does. Returns the number
/// of records inserted.
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

    let client_id = config
        .oura_client_id
        .as_deref()
        .ok_or(SyncError::NotConfigured)?;
    let client_secret = config
        .oura_client_secret
        .as_deref()
        .ok_or(SyncError::NotConfigured)?;

    let client = OuraClient::new(
        client_id.to_string(),
        client_secret.to_string(),
        config.oura_api_base_url.clone(),
        config.oura_auth_base_url.clone(),
        http_client.clone(),
    );

    let token_row =
        integration_tokens::list_for_user(pool, user_id, &encryption_key, prev_key.as_ref())
            .await
            .map_err(|e| SyncError::Upstream(format!("failed to load integration tokens: {e}")))?
            .into_iter()
            .find(|t| t.source == SOURCE)
            .ok_or(SyncError::NotConnected)?;

    // Cooldown: reject if a *prior* attempt (success or failure, manual or
    // scheduled) completed too recently. `updated_at` is bumped by both
    // `update_last_synced` and `update_sync_error`, so it reflects the last
    // attempt either way — this protects our shared Oura app quota from an
    // abusive client loop. Skipped entirely the first time a freshly-connected
    // account is synced (no `last_synced_at`/`last_sync_error` yet) so
    // connecting and immediately syncing always works.
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
        sync_user(pool, &client, &token_row, &encryption_key, event_tx)
    })
    .await
    .map_err(|e| SyncError::Upstream(format!("failed to acquire sync lock: {e}")))?;

    let outcome = match lock_result {
        Some(outcome) => outcome,
        // Another sync for this user is already running (a concurrent manual
        // sync, or the periodic job landed on this user first). A short fixed
        // retry — the other sync will very likely have finished by then.
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

/// Sync data for a single user. Returns the number of records inserted on
/// success. `last_synced_at` is advanced only when every fetch succeeds — a
/// partial failure returns `Err` so the caller records `last_sync_error`
/// instead, leaving the watermark where it was so the failed window is
/// retried on the next run rather than silently skipped forever.
async fn sync_user(
    pool: &PgPool,
    client: &OuraClient,
    token_row: &integration_tokens::IntegrationTokenRow,
    encryption_key: &[u8; 32],
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<u32, String> {
    let user_id = token_row.user_id;

    let mut access_token = token_row.access_token.clone();

    // Check if the token has expired and refresh if needed.
    if let Some(expires_at) = token_row.expires_at
        && expires_at < Utc::now()
    {
        let refresh_token = token_row
            .refresh_token
            .as_deref()
            .ok_or("Oura token expired and no refresh token available")?;

        let new_tokens = client
            .refresh_token(refresh_token)
            .await
            .map_err(|e| format!("Oura token refresh failed: {e}"))?;

        let new_expires_at = new_tokens
            .expires_in
            .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

        integration_tokens::upsert(
            pool,
            user_id,
            "oura",
            &new_tokens.access_token,
            new_tokens.refresh_token.as_deref(),
            new_expires_at,
            encryption_key,
        )
        .await
        .map_err(|e| format!("failed to update Oura tokens after refresh: {e}"))?;

        access_token = new_tokens.access_token;
    }

    // Determine the date range to sync.
    let start_date = token_row
        .last_synced_at
        .map(|ts| ts.date_naive())
        .unwrap_or_else(|| (Utc::now() - Duration::days(7)).date_naive());
    let end_date = Utc::now().date_naive();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    let mut records_inserted = 0u32;
    let mut fetch_errors = Vec::new();

    // Fetch daily readiness
    match client
        .get_daily_readiness(&access_token, &start_str, &end_str)
        .await
    {
        Ok(response) => {
            for readiness in response.data {
                records_inserted += insert_readiness_records(pool, user_id, &readiness).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Oura readiness fetch failed");
            fetch_errors.push(format!("readiness: {e}"));
        }
    }

    // Fetch daily sleep
    match client
        .get_daily_sleep(&access_token, &start_str, &end_str)
        .await
    {
        Ok(response) => {
            for sleep in response.data {
                records_inserted += insert_oura_sleep(pool, user_id, &sleep).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Oura sleep fetch failed");
            fetch_errors.push(format!("sleep: {e}"));
        }
    }

    // Fetch daily activity
    match client
        .get_daily_activity(&access_token, &start_str, &end_str)
        .await
    {
        Ok(response) => {
            for activity in response.data {
                records_inserted += insert_activity_records(pool, user_id, &activity).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Oura activity fetch failed");
            fetch_errors.push(format!("activity: {e}"));
        }
    }

    // Notify listeners of newly inserted data even if some fetches failed
    // below — the records that did land are real and should surface.
    if records_inserted > 0 {
        let _ = event_tx.send((
            user_id,
            crate::models::explore::DataChangedEvent {
                source: "oura".to_string(),
                record_type: None,
            },
        ));
    }

    if !fetch_errors.is_empty() {
        return Err(format!(
            "{} of 3 Oura fetches failed: {}",
            fetch_errors.len(),
            fetch_errors.join("; ")
        ));
    }

    // Only advance the watermark once every fetch succeeded — a partial
    // failure above returns before this point, so `last_synced_at` stays put
    // and the failed window is retried next run instead of lost.
    integration_tokens::update_last_synced(pool, user_id, "oura")
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    tracing::info!(user_id = %user_id, records = records_inserted, "Oura sync completed");

    Ok(records_inserted)
}

/// Insert health records from Oura readiness data. Returns count inserted.
async fn insert_readiness_records(
    pool: &PgPool,
    user_id: Uuid,
    readiness: &crate::integrations::oura::OuraReadiness,
) -> u32 {
    let date_str = match readiness.day.as_deref() {
        Some(d) => d,
        None => return 0,
    };
    let date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let start_time = date
        .and_hms_opt(0, 0, 0)
        .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        .unwrap_or_else(Utc::now);

    let mut count = 0u32;

    // HRV balance from contributors
    if let Some(ref contributors) = readiness.contributors {
        if let Some(hrv) = contributors.hrv_balance {
            let record = CreateHealthRecord {
                source: "oura".to_string(),
                record_type: "heart_rate_variability".to_string(),
                value: Some(hrv),
                unit: Some("ms".to_string()),
                start_time,
                end_time: None,
                metadata: None,
                source_id: Some(format!("oura-hrv-{date_str}")),
            };
            if try_insert_health_record(pool, user_id, &record).await {
                count += 1;
            }
        }

        if let Some(temp) = contributors.body_temperature {
            let record = CreateHealthRecord {
                source: "oura".to_string(),
                record_type: "body_temperature".to_string(),
                value: Some(temp),
                unit: Some("celsius_delta".to_string()),
                start_time,
                end_time: None,
                metadata: None,
                source_id: Some(format!("oura-temp-{date_str}")),
            };
            if try_insert_health_record(pool, user_id, &record).await {
                count += 1;
            }
        }

        if let Some(rhr) = contributors.resting_heart_rate {
            let record = CreateHealthRecord {
                source: "oura".to_string(),
                record_type: "resting_heart_rate".to_string(),
                value: Some(rhr),
                unit: Some("bpm".to_string()),
                start_time,
                end_time: None,
                metadata: None,
                source_id: Some(format!("oura-rhr-{date_str}")),
            };
            if try_insert_health_record(pool, user_id, &record).await {
                count += 1;
            }
        }
    }

    count
}

/// Insert a sleep observation from Oura sleep data. Returns count inserted.
async fn insert_oura_sleep(
    pool: &PgPool,
    user_id: Uuid,
    sleep: &crate::integrations::oura::OuraSleep,
) -> u32 {
    let date_str = match sleep.day.as_deref() {
        Some(d) => d,
        None => return 0,
    };

    let start_time = sleep
        .bedtime_start
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(22, 0, 0))
                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now)
        });

    let end_time = sleep
        .bedtime_end
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let value = serde_json::json!({
        "deep_sleep_duration": sleep.deep_sleep_duration,
        "light_sleep_duration": sleep.light_sleep_duration,
        "rem_sleep_duration": sleep.rem_sleep_duration,
        "awake_time": sleep.awake_time,
        "total_sleep_duration": sleep.total_sleep_duration,
        "score": sleep.score,
        "average_heart_rate": sleep.average_heart_rate,
        "lowest_heart_rate": sleep.lowest_heart_rate,
    });

    let obs = CreateObservation {
        obs_type: "sleep".to_string(),
        name: "oura_sleep".to_string(),
        start_time,
        end_time,
        value: Some(value),
        source: Some("oura".to_string()),
        metadata: None,
    };

    // Deterministic per-night id so a re-sync of the same night is a no-op
    // (`insert_synced`) rather than a fresh duplicate row every 15 minutes.
    let source_id = format!("oura-sleep-{date_str}");

    match observations::insert_synced(pool, user_id, &obs, &source_id).await {
        Ok(Some(_)) => 1,
        Ok(None) => 0, // already synced — not an error, don't warn
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "failed to insert Oura sleep observation");
            0
        }
    }
}

/// Insert health records from Oura activity data. Returns count inserted.
async fn insert_activity_records(
    pool: &PgPool,
    user_id: Uuid,
    activity: &crate::integrations::oura::OuraActivity,
) -> u32 {
    let date_str = match activity.day.as_deref() {
        Some(d) => d,
        None => return 0,
    };
    let date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return 0,
    };
    let start_time = date
        .and_hms_opt(0, 0, 0)
        .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        .unwrap_or_else(Utc::now);

    let mut count = 0u32;

    if let Some(steps) = activity.steps {
        let record = CreateHealthRecord {
            source: "oura".to_string(),
            record_type: "steps".to_string(),
            value: Some(steps as f64),
            unit: Some("count".to_string()),
            start_time,
            end_time: None,
            metadata: None,
            source_id: Some(format!("oura-steps-{date_str}")),
        };
        if try_insert_health_record(pool, user_id, &record).await {
            count += 1;
        }
    }

    count
}

/// Try to insert a health record, checking for duplicates first.
/// Returns true if inserted, false if skipped or error.
async fn try_insert_health_record(
    pool: &PgPool,
    user_id: Uuid,
    record: &CreateHealthRecord,
) -> bool {
    // Check for a cross-source duplicate (different source, same value/time
    // window — see CLAUDE.md's dedup rule). Either way, the actual write
    // below goes through `insert_synced`: every Oura record carries a
    // deterministic `source_id`, so re-syncing the same day is a normal `None`
    // outcome, not a unique-violation warning on every 15-minute cycle.
    let duplicate_of = match health_records::find_duplicate(pool, user_id, record).await {
        Ok(Some(existing)) => {
            tracing::warn!(
                user_id = %user_id,
                existing_id = %existing.id,
                existing_source = %existing.source,
                new_source = %record.source,
                record_type = %record.record_type,
                "duplicate health record detected from Oura sync"
            );
            Some(existing.id)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, "failed to check for duplicate health record");
            return false;
        }
    };

    match health_records::insert_synced(pool, user_id, record, duplicate_of).await {
        Ok(Some(_)) => true,
        Ok(None) => false, // already synced — not an error, don't warn
        Err(e) => {
            tracing::warn!(error = %e, record_type = %record.record_type, "failed to insert health record from Oura");
            false
        }
    }
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
