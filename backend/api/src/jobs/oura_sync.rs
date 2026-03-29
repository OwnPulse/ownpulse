// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync job for Oura Ring API data.
//!
//! Periodically fetches readiness, sleep, activity, and heart rate data
//! from Oura for all users with connected Oura integrations.

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{health_records, integration_tokens, observations};
use crate::integrations::oura::OuraClient;
use crate::models::health_record::CreateHealthRecord;
use crate::models::observation::CreateObservation;

/// Interval between sync runs (15 minutes).
const SYNC_INTERVAL_SECS: u64 = 900;

/// Spawn the Oura sync background job.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) {
    tokio::spawn(async move {
        tracing::info!("Oura sync job started");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Oura sync job shutting down");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(SYNC_INTERVAL_SECS)) => {
                    if let Err(e) = run_sync(&pool, &config, &http_client, &event_tx).await {
                        tracing::error!(error = %e, "Oura sync run failed");
                    }
                }
            }
        }
    });
}

/// Run a single sync cycle for all users with Oura integration tokens.
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
        "oura",
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list Oura tokens: {e}"))?;

    for token_row in tokens {
        let user_id = token_row.user_id;
        if let Err(e) = sync_user(pool, &client, &token_row, &encryption_key, event_tx).await {
            tracing::error!(user_id = %user_id, error = %e, "Oura sync failed for user");
            let _ = integration_tokens::update_sync_error(pool, user_id, "oura", &e).await;
        }
    }

    Ok(())
}

/// Sync data for a single user.
async fn sync_user(
    pool: &PgPool,
    client: &OuraClient,
    token_row: &integration_tokens::IntegrationTokenRow,
    encryption_key: &[u8; 32],
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) -> Result<(), String> {
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
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Oura readiness fetch failed"),
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
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Oura sleep fetch failed"),
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
        Err(e) => tracing::warn!(user_id = %user_id, error = %e, "Oura activity fetch failed"),
    }

    // Update last_synced_at
    integration_tokens::update_last_synced(pool, user_id, "oura")
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    if records_inserted > 0 {
        let _ = event_tx.send((
            user_id,
            crate::models::explore::DataChangedEvent {
                source: "oura".to_string(),
                record_type: None,
            },
        ));
    }

    tracing::info!(user_id = %user_id, records = records_inserted, "Oura sync completed");

    Ok(())
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

    match observations::insert(pool, user_id, &obs).await {
        Ok(_) => 1,
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
    match health_records::find_duplicate(pool, user_id, record).await {
        Ok(Some(existing)) => {
            tracing::warn!(
                user_id = %user_id,
                existing_id = %existing.id,
                existing_source = %existing.source,
                new_source = %record.source,
                record_type = %record.record_type,
                "duplicate health record detected from Oura sync"
            );
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
                tracing::warn!(error = %e, record_type = %record.record_type, "failed to insert health record from Oura");
                false
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "failed to check for duplicate health record");
            false
        }
    }
}
