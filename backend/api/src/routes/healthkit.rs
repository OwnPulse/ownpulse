// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use metrics::{counter, histogram};

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::health_records as db_hr;
use crate::db::healthkit as db;
use crate::error::ApiError;
use crate::models::health_record::HealthRecordRow;
use crate::models::healthkit::{HealthKitBulkInsert, HealthKitConfirm, HealthKitWriteQueueRow};

/// POST /healthkit/sync — bulk insert health records from HealthKit in one
/// round trip.
///
/// Uses a set-based `INSERT ... SELECT FROM UNNEST(...)` with
/// `ON CONFLICT DO NOTHING` on `UNIQUE(user_id, source, record_type,
/// start_time, source_id)` for same-source idempotency. Source is forced to
/// `"healthkit"` in the SQL regardless of what the client sends.
///
/// Cross-source deduplication (`duplicate_of`) is **not** computed here —
/// synchronous cross-source dedup made batches of 100 records take ~1s (200
/// DB round trips per batch). That reconciliation is deferred to a future
/// async job; rows inserted via this path leave `duplicate_of = NULL`.
///
/// Response body is an empty JSON array: iOS only awaits completion and does
/// not read individual rows back from this endpoint.
pub async fn bulk_insert(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<HealthKitBulkInsert>,
) -> Result<(StatusCode, Json<Vec<HealthRecordRow>>), ApiError> {
    histogram!("healthkit_sync_batch_size").record(body.records.len() as f64);

    let inserted = db_hr::bulk_insert_healthkit(&state.pool, user_id, &body.records).await?;

    // We've lost per-record-type granularity by batching — report a single
    // counter labelled only by source. Per-type breakdown can be re-added
    // later via a deferred async dedup/reconciliation job.
    counter!("healthkit_records_ingested_total", "source" => "healthkit")
        .increment(inserted as u64);

    Ok((StatusCode::CREATED, Json(Vec::new())))
}

/// GET /healthkit/write-queue — pending items for the iOS client to write to HealthKit.
pub async fn write_queue(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<HealthKitWriteQueueRow>>, ApiError> {
    let rows = db::get_pending(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /healthkit/confirm — confirm that items have been written to HealthKit.
pub async fn confirm(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<HealthKitConfirm>,
) -> Result<StatusCode, ApiError> {
    db::confirm(&state.pool, user_id, &body.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
