// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background + on-demand sync for MyChart / SMART-on-FHIR lab data.
//!
//! Fetches FHIR `Observation` (laboratory) resources for every user with a
//! connected MyChart integration and imports them into `lab_results`. Imports
//! are idempotent: each lab row carries the FHIR resource id as `source_id`,
//! and `lab_results.bulk_insert` skips rows that already exist via the dedup
//! unique index.
//!
//! Lab data is health data — imported verbatim. We never validate, filter, or
//! judge marker names or values.

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto;
use crate::db::{integration_tokens, lab_results};
use crate::integrations::mychart::{self, MyChartClient};

/// Interval between background sync runs (6 hours — lab results change slowly).
const SYNC_INTERVAL_SECS: u64 = 6 * 60 * 60;

/// Result of a single user's sync.
pub struct SyncOutcome {
    pub imported: u32,
    pub skipped: u32,
}

/// Spawn the MyChart background sync job.
pub fn spawn(
    pool: PgPool,
    config: Config,
    http_client: reqwest::Client,
    cancel: CancellationToken,
    event_tx: tokio::sync::broadcast::Sender<(Uuid, crate::models::explore::DataChangedEvent)>,
) {
    tokio::spawn(async move {
        tracing::info!("MyChart sync job started");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("MyChart sync job shutting down");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(SYNC_INTERVAL_SECS)) => {
                    if let Err(e) = run_sync(&pool, &config, &http_client, &event_tx).await {
                        tracing::error!(error = %e, "MyChart sync run failed");
                    }
                }
            }
        }
    });
}

/// Run a single sync cycle for all users with a MyChart connection.
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

    if config.mychart_client_id.is_none() {
        return Ok(()); // MyChart not configured, skip.
    }

    let tokens = integration_tokens::list_for_user_by_source(
        pool,
        mychart::SOURCE,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| format!("failed to list MyChart tokens: {e}"))?;

    for token_row in tokens {
        let user_id = token_row.user_id;
        match sync_token_row(pool, config, http_client, &token_row, &encryption_key).await {
            Ok(outcome) if outcome.imported > 0 => {
                let _ = event_tx.send((
                    user_id,
                    crate::models::explore::DataChangedEvent {
                        source: mychart::SOURCE.to_string(),
                        record_type: Some("lab_result".to_string()),
                    },
                ));
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(user_id = %user_id, "MyChart sync failed for user");
                let _ =
                    integration_tokens::update_sync_error(pool, user_id, mychart::SOURCE, &e).await;
            }
        }
    }

    Ok(())
}

/// On-demand sync for a single user — used by the `/integrations/mychart/sync`
/// endpoint. Returns how many lab rows were imported vs. skipped (duplicates).
pub async fn sync_user_now(
    pool: &PgPool,
    config: &Config,
    http_client: &reqwest::Client,
    user_id: Uuid,
) -> Result<SyncOutcome, String> {
    let encryption_key = crypto::parse_encryption_key(&config.encryption_key)
        .map_err(|e| format!("bad encryption key: {e}"))?;
    let prev_key = config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()
        .map_err(|e| format!("bad previous encryption key: {e}"))?;

    let token_row =
        integration_tokens::list_for_user(pool, user_id, &encryption_key, prev_key.as_ref())
            .await
            .map_err(|e| format!("failed to load integration tokens: {e}"))?
            .into_iter()
            .find(|t| t.source == mychart::SOURCE)
            .ok_or_else(|| "MyChart is not connected".to_string())?;

    let outcome = sync_token_row(pool, config, http_client, &token_row, &encryption_key).await;
    if let Err(ref e) = outcome {
        let _ = integration_tokens::update_sync_error(pool, user_id, mychart::SOURCE, e).await;
    }
    outcome
}

/// Sync a single connection: refresh the token if expired, fetch lab
/// observations, parse them, and bulk-insert into `lab_results`.
async fn sync_token_row(
    pool: &PgPool,
    config: &Config,
    http_client: &reqwest::Client,
    token_row: &integration_tokens::IntegrationTokenRow,
    encryption_key: &[u8; 32],
) -> Result<SyncOutcome, String> {
    let user_id = token_row.user_id;

    let client_id = config
        .mychart_client_id
        .as_deref()
        .ok_or("MYCHART_CLIENT_ID not configured")?;

    // The FHIR base URL and token endpoint are stored as connection metadata
    // at connect time.
    let metadata = token_row
        .metadata
        .as_ref()
        .ok_or("MyChart connection is missing FHIR metadata")?;
    let fhir_base_url = metadata
        .get("fhir_base_url")
        .and_then(|v| v.as_str())
        .ok_or("MyChart metadata missing fhir_base_url")?;
    let token_endpoint = metadata
        .get("token_endpoint")
        .and_then(|v| v.as_str())
        .ok_or("MyChart metadata missing token_endpoint")?;

    let client = MyChartClient::new(
        client_id.to_string(),
        token_endpoint.to_string(),
        fhir_base_url.to_string(),
        http_client.clone(),
    );

    let mut access_token = token_row.access_token.clone();

    // Refresh the access token if it has expired.
    if let Some(expires_at) = token_row.expires_at
        && expires_at < chrono::Utc::now()
    {
        let refresh_token = token_row
            .refresh_token
            .as_deref()
            .ok_or("MyChart token expired and no refresh token available")?;

        let new_tokens = client
            .refresh_token(refresh_token)
            .await
            .map_err(|e| format!("MyChart token refresh failed: {e}"))?;

        let new_expires_at = new_tokens
            .expires_in
            .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs));

        integration_tokens::upsert_with_metadata(
            pool,
            user_id,
            mychart::SOURCE,
            &new_tokens.access_token,
            new_tokens.refresh_token.as_deref(),
            new_expires_at,
            token_row.metadata.as_ref(),
            encryption_key,
        )
        .await
        .map_err(|e| format!("failed to update MyChart tokens after refresh: {e}"))?;

        access_token = new_tokens.access_token;
    }

    let bundle = client
        .get_lab_observations(&access_token)
        .await
        .map_err(|e| format!("failed to fetch MyChart observations: {e}"))?;

    let labs = mychart::parse_observation_bundle(&bundle);
    let parsed = labs.len() as u32;

    let inserted = lab_results::bulk_insert(pool, user_id, &labs)
        .await
        .map_err(|e| format!("failed to insert MyChart lab results: {e}"))?;
    let imported = inserted.len() as u32;

    integration_tokens::update_last_synced(pool, user_id, mychart::SOURCE)
        .await
        .map_err(|e| format!("failed to update last_synced_at: {e}"))?;

    tracing::info!(user_id = %user_id, imported, "MyChart sync completed");

    Ok(SyncOutcome {
        imported,
        skipped: parsed.saturating_sub(imported),
    })
}
