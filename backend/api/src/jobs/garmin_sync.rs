// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync job for Garmin Health API data.
//!
//! Periodically fetches daily summaries, sleep, HRV, and body composition
//! data from Garmin for all users with connected Garmin integrations.

use std::time::Duration as StdDuration;

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{health_records, integration_tokens, observations};
use crate::integrations::garmin::{AccessToken, GarminClient};
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
const SOURCE: &str = "garmin";

/// Spawn the Garmin sync background job. Returns the task handle so callers
/// (and tests) can observe shutdown; `main.rs` does not need to await it.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("Garmin sync job started");

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
                    tracing::info!("Garmin sync job shutting down");
                    break;
                }
                _ = interval.tick() => {
                    // Race the sync pass itself against cancellation so a slow
                    // provider (or many connected users) can't delay shutdown
                    // past the in-flight HTTP request's timeout — dropping the
                    // future here aborts whatever request/query was pending.
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            tracing::info!("Garmin sync job cancelled mid-cycle");
                            break;
                        }
                        result = run_sync(&pool, &config, &http_client, &event_tx, &cancel) => {
                            if let Err(e) = result {
                                tracing::error!(error = %e, "Garmin sync run failed");
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Run a single sync cycle for all users with Garmin integration tokens.
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

    let consumer_key = match config.garmin_client_id.as_deref() {
        Some(k) => k,
        None => return Ok(()), // Garmin not configured, skip
    };
    let consumer_secret = match config.garmin_client_secret.as_deref() {
        Some(s) => s,
        None => return Ok(()),
    };

    let client = GarminClient::new(
        consumer_key.to_string(),
        consumer_secret.to_string(),
        config.garmin_base_url.clone(),
        http_client.clone(),
    );

    let tokens = integration_tokens::list_for_user_by_source(
        pool,
        SOURCE,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list Garmin tokens: {e}"))?;

    for token_row in tokens {
        if cancel.is_cancelled() {
            tracing::info!("Garmin sync pass interrupted by shutdown");
            break;
        }

        let user_id = token_row.user_id;

        // Advisory lock keyed on (source, user_id): skip (don't block) if
        // another sync for this user is already running — a concurrent
        // manual sync, or (with `replicaCount: 2`) another replica's
        // periodic job iterating the same rows.
        let lock_result = try_with_user_sync_lock(pool, SOURCE, user_id, || {
            sync_user(pool, &client, &token_row, event_tx)
        })
        .await;

        match lock_result {
            Ok(Some(Err(e))) => {
                tracing::error!(user_id = %user_id, error = %e, "Garmin sync failed for user");
                let _ = integration_tokens::update_sync_error(pool, user_id, SOURCE, &e).await;
            }
            Ok(Some(Ok(_))) => {}
            Ok(None) => {
                tracing::debug!(user_id = %user_id, "skipping Garmin sync — already in progress elsewhere");
            }
            Err(e) => {
                tracing::error!(user_id = %user_id, error = %e, "failed to acquire Garmin sync lock");
            }
        }
    }

    Ok(())
}

/// On-demand sync for a single user — used by the
/// `POST /integrations/garmin/sync` endpoint. Looks up the user's Garmin
/// connection, runs one sync cycle, and records the outcome the same way the
/// periodic job does. Returns the number of records inserted.
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

    let consumer_key = config
        .garmin_client_id
        .as_deref()
        .ok_or(SyncError::NotConfigured)?;
    let consumer_secret = config
        .garmin_client_secret
        .as_deref()
        .ok_or(SyncError::NotConfigured)?;

    let client = GarminClient::new(
        consumer_key.to_string(),
        consumer_secret.to_string(),
        config.garmin_base_url.clone(),
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
    // attempt either way — this protects our shared Garmin app quota (a
    // human-reviewed developer registration) from an abusive client loop.
    // Skipped entirely the first time a freshly-connected account is synced
    // (no `last_synced_at`/`last_sync_error` yet) so connecting and
    // immediately syncing always works.
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
        sync_user(pool, &client, &token_row, event_tx)
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
    client: &GarminClient,
    token_row: &integration_tokens::IntegrationTokenRow,
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<u32, String> {
    let user_id = token_row.user_id;

    let access_token = AccessToken {
        oauth_token: token_row.access_token.clone(),
        oauth_token_secret: token_row.refresh_token.clone().unwrap_or_default(),
    };

    // Determine the date range to sync: since last sync or last 7 days.
    let start_date = token_row
        .last_synced_at
        .map(|ts| ts.date_naive())
        .unwrap_or_else(|| (Utc::now() - Duration::days(7)).date_naive());
    let end_date = Utc::now().date_naive();

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    let mut records_inserted = 0u32;
    let mut fetch_errors = Vec::new();

    // Fetch daily summaries
    match client
        .get_daily_summary(&access_token, &start_str, &end_str)
        .await
    {
        Ok(summaries) => {
            for summary in summaries {
                records_inserted += insert_daily_summary_records(pool, user_id, &summary).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Garmin daily summary fetch failed");
            fetch_errors.push(format!("daily summary: {e}"));
        }
    }

    // Fetch sleep data
    match client.get_sleep(&access_token, &start_str, &end_str).await {
        Ok(sleeps) => {
            for sleep in sleeps {
                records_inserted += insert_sleep_observation(pool, user_id, &sleep).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Garmin sleep fetch failed");
            fetch_errors.push(format!("sleep: {e}"));
        }
    }

    // Fetch HRV data
    match client.get_hrv(&access_token, &start_str, &end_str).await {
        Ok(hrvs) => {
            for hrv in hrvs {
                records_inserted += insert_hrv_record(pool, user_id, &hrv).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Garmin HRV fetch failed");
            fetch_errors.push(format!("hrv: {e}"));
        }
    }

    // Fetch body composition
    match client
        .get_body_comp(&access_token, &start_str, &end_str)
        .await
    {
        Ok(body_comps) => {
            for bc in body_comps {
                records_inserted += insert_body_comp_records(pool, user_id, &bc).await;
            }
        }
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Garmin body comp fetch failed");
            fetch_errors.push(format!("body composition: {e}"));
        }
    }

    // Notify listeners of newly inserted data even if some fetches failed
    // below — the records that did land are real and should surface.
    if records_inserted > 0 {
        let _ = event_tx.send((
            user_id,
            crate::models::explore::DataChangedEvent {
                source: "garmin".to_string(),
                record_type: None,
            },
        ));
    }

    if !fetch_errors.is_empty() {
        return Err(format!(
            "{} of 4 Garmin fetches failed: {}",
            fetch_errors.len(),
            fetch_errors.join("; ")
        ));
    }

    // Only advance the watermark once every fetch succeeded — a partial
    // failure above returns before this point, so `last_synced_at` stays put
    // and the failed window is retried next run instead of lost.
    integration_tokens::update_last_synced(pool, user_id, "garmin")
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    tracing::info!(user_id = %user_id, records = records_inserted, "Garmin sync completed");

    Ok(records_inserted)
}

/// Insert health_records from a Garmin daily summary. Returns count inserted.
async fn insert_daily_summary_records(
    pool: &PgPool,
    user_id: Uuid,
    summary: &crate::integrations::garmin::GarminDailySummary,
) -> u32 {
    let date_str = match summary.calendar_date.as_deref() {
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

    if let Some(steps) = summary.total_steps {
        let record = CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "steps".to_string(),
            value: Some(steps as f64),
            unit: Some("count".to_string()),
            start_time,
            end_time: None,
            metadata: None,
            source_id: Some(format!("garmin-steps-{date_str}")),
        };
        if try_insert_health_record(pool, user_id, &record).await {
            count += 1;
        }
    }

    if let Some(rhr) = summary.resting_heart_rate {
        let record = CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "resting_heart_rate".to_string(),
            value: Some(rhr),
            unit: Some("bpm".to_string()),
            start_time,
            end_time: None,
            metadata: None,
            source_id: Some(format!("garmin-rhr-{date_str}")),
        };
        if try_insert_health_record(pool, user_id, &record).await {
            count += 1;
        }
    }

    count
}

/// Insert a sleep observation from Garmin sleep data. Returns count inserted.
async fn insert_sleep_observation(
    pool: &PgPool,
    user_id: Uuid,
    sleep: &crate::integrations::garmin::GarminSleep,
) -> u32 {
    let date_str = match sleep.calendar_date.as_deref() {
        Some(d) => d,
        None => return 0,
    };

    let start_time = sleep
        .sleep_start_timestamp_gmt
        .and_then(|ts| chrono::DateTime::from_timestamp(ts / 1000, 0))
        .unwrap_or_else(|| {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(22, 0, 0))
                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now)
        });

    let end_time = sleep
        .sleep_end_timestamp_gmt
        .and_then(|ts| chrono::DateTime::from_timestamp(ts / 1000, 0));

    let value = serde_json::json!({
        "deep_sleep_seconds": sleep.deep_sleep_seconds,
        "light_sleep_seconds": sleep.light_sleep_seconds,
        "rem_sleep_seconds": sleep.rem_sleep_seconds,
        "awake_sleep_seconds": sleep.awake_sleep_seconds,
        "overall_score": sleep.overall_score,
    });

    let obs = CreateObservation {
        obs_type: "sleep".to_string(),
        name: "garmin_sleep".to_string(),
        start_time,
        end_time,
        value: Some(value),
        source: Some("garmin".to_string()),
        metadata: None,
    };

    // Deterministic per-night id so a re-sync of the same night is a no-op
    // (`insert_synced`) rather than a fresh duplicate row every 15 minutes.
    let source_id = format!("garmin-sleep-{date_str}");

    match observations::insert_synced(pool, user_id, &obs, &source_id).await {
        Ok(Some(_)) => 1,
        Ok(None) => 0, // already synced — not an error, don't warn
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "failed to insert Garmin sleep observation");
            0
        }
    }
}

/// Insert an HRV health_record from Garmin. Returns count inserted.
async fn insert_hrv_record(
    pool: &PgPool,
    user_id: Uuid,
    hrv: &crate::integrations::garmin::GarminHrv,
) -> u32 {
    let date_str = match hrv.calendar_date.as_deref() {
        Some(d) => d,
        None => return 0,
    };
    let hrv_value = match hrv.last_night.or(hrv.weekly_avg) {
        Some(v) => v,
        None => return 0,
    };

    let start_time = hrv
        .start_timestamp_gmt
        .and_then(|ts| chrono::DateTime::from_timestamp(ts / 1000, 0))
        .unwrap_or_else(|| {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now)
        });

    let record = CreateHealthRecord {
        source: "garmin".to_string(),
        record_type: "heart_rate_variability".to_string(),
        value: Some(hrv_value),
        unit: Some("ms".to_string()),
        start_time,
        end_time: None,
        metadata: None,
        source_id: Some(format!("garmin-hrv-{date_str}")),
    };

    if try_insert_health_record(pool, user_id, &record).await {
        1
    } else {
        0
    }
}

/// Insert body composition health_records from Garmin. Returns count inserted.
async fn insert_body_comp_records(
    pool: &PgPool,
    user_id: Uuid,
    bc: &crate::integrations::garmin::GarminBodyComp,
) -> u32 {
    let date_str = match bc.calendar_date.as_deref() {
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

    if let Some(weight) = bc.weight {
        // Garmin reports weight in grams, convert to kg.
        let weight_kg = weight / 1000.0;
        let record = CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "body_mass".to_string(),
            value: Some(weight_kg),
            unit: Some("kg".to_string()),
            start_time,
            end_time: None,
            metadata: None,
            source_id: Some(format!("garmin-weight-{date_str}")),
        };
        if try_insert_health_record(pool, user_id, &record).await {
            count += 1;
        }
    }

    if let Some(body_fat) = bc.body_fat {
        let record = CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "body_fat_percentage".to_string(),
            value: Some(body_fat),
            unit: Some("%".to_string()),
            start_time,
            end_time: None,
            metadata: None,
            source_id: Some(format!("garmin-bodyfat-{date_str}")),
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
    // below goes through `insert_synced`: every Garmin record carries a
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
                "duplicate health record detected from Garmin sync"
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
            tracing::warn!(error = %e, record_type = %record.record_type, "failed to insert health record from Garmin");
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
