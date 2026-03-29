// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync job for Garmin Health API data.
//!
//! Periodically fetches daily summaries, sleep, HRV, and body composition
//! data from Garmin for all users with connected Garmin integrations.

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{health_records, integration_tokens, observations};
use crate::integrations::garmin::{AccessToken, GarminClient};
use crate::models::health_record::CreateHealthRecord;
use crate::models::observation::CreateObservation;

/// Interval between sync runs (15 minutes).
const SYNC_INTERVAL_SECS: u64 = 900;

/// Spawn the Garmin sync background job.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) {
    tokio::spawn(async move {
        tracing::info!("Garmin sync job started");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Garmin sync job shutting down");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(SYNC_INTERVAL_SECS)) => {
                    if let Err(e) = run_sync(&pool, &config, &http_client, &event_tx).await {
                        tracing::error!(error = %e, "Garmin sync run failed");
                    }
                }
            }
        }
    });
}

/// Run a single sync cycle for all users with Garmin integration tokens.
async fn run_sync(
    pool: &PgPool,
    config: &Config,
    http_client: &reqwest::Client,
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
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
        "garmin",
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list Garmin tokens: {e}"))?;

    for token_row in tokens {
        let user_id = token_row.user_id;
        if let Err(e) = sync_user(pool, &client, &token_row, event_tx).await {
            tracing::error!(user_id = %user_id, error = %e, "Garmin sync failed for user");
            let _ = integration_tokens::update_sync_error(pool, user_id, "garmin", &e).await;
        }
    }

    Ok(())
}

/// Sync data for a single user.
async fn sync_user(
    pool: &PgPool,
    client: &GarminClient,
    token_row: &integration_tokens::IntegrationTokenRow,
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<(), String> {
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
            tracing::warn!(user_id = %user_id, error = %e, "Garmin daily summary fetch failed")
        }
    }

    // Fetch sleep data
    match client.get_sleep(&access_token, &start_str, &end_str).await {
        Ok(sleeps) => {
            for sleep in sleeps {
                records_inserted += insert_sleep_observation(pool, user_id, &sleep).await;
            }
        }
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Garmin sleep fetch failed"),
    }

    // Fetch HRV data
    match client.get_hrv(&access_token, &start_str, &end_str).await {
        Ok(hrvs) => {
            for hrv in hrvs {
                records_inserted += insert_hrv_record(pool, user_id, &hrv).await;
            }
        }
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Garmin HRV fetch failed"),
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
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Garmin body comp fetch failed"),
    }

    // Update last_synced_at
    integration_tokens::update_last_synced(pool, user_id, "garmin")
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    if records_inserted > 0 {
        let _ = event_tx.send((
            user_id,
            crate::models::explore::DataChangedEvent {
                source: "garmin".to_string(),
                record_type: None,
            },
        ));
    }

    tracing::info!(user_id = %user_id, records = records_inserted, "Garmin sync completed");

    Ok(())
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

    match observations::insert(pool, user_id, &obs).await {
        Ok(_) => 1,
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
    // Check for duplicates
    match health_records::find_duplicate(pool, user_id, record).await {
        Ok(Some(existing)) => {
            tracing::warn!(
                user_id = %user_id,
                existing_id = %existing.id,
                existing_source = %existing.source,
                new_source = %record.source,
                record_type = %record.record_type,
                "duplicate health record detected from Garmin sync"
            );
            // Insert with duplicate_of reference
            match health_records::insert(pool, user_id, record, Some(existing.id)).await {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to insert duplicate-linked health record");
                    false
                }
            }
        }
        Ok(None) => match health_records::insert(pool, user_id, record, None).await {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(error = %e, record_type = %record.record_type, "failed to insert health record from Garmin");
                false
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to check for duplicate health record");
            false
        }
    }
}
