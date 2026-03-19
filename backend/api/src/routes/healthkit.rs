// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::auth::extractor::AuthUser;
use crate::db::health_records as db_hr;
use crate::db::healthkit as db;
use crate::error::ApiError;
use crate::models::health_record::HealthRecordRow;
use crate::models::healthkit::{HealthKitBulkInsert, HealthKitConfirm, HealthKitWriteQueueRow};
use crate::AppState;

/// POST /healthkit/sync — bulk insert health records from HealthKit.
/// Forces source="healthkit" on every record. Never enqueues write-back.
pub async fn bulk_insert(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<HealthKitBulkInsert>,
) -> Result<(StatusCode, Json<Vec<HealthRecordRow>>), ApiError> {
    let mut inserted = Vec::with_capacity(body.records.len());

    for mut record in body.records {
        // Unconditionally force source to "healthkit"
        record.source = "healthkit".to_string();

        // Check for duplicates
        let duplicate_of = match db_hr::find_duplicate(&state.pool, user_id, &record).await {
            Ok(Some(dup)) => {
                tracing::warn!(
                    user_id = %user_id,
                    existing_id = %dup.id,
                    existing_source = %dup.source,
                    new_source = "healthkit",
                    record_type = %record.record_type,
                    "duplicate health record detected during healthkit sync"
                );
                Some(dup.id)
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "failed to check for duplicate during healthkit sync");
                None
            }
        };

        let row = db_hr::insert(&state.pool, user_id, &record, duplicate_of).await?;
        // Never enqueue write-back for healthkit-sourced records
        inserted.push(row);
    }

    Ok((StatusCode::CREATED, Json(inserted)))
}

/// GET /healthkit/write-queue — pending items for the iOS client to write to HealthKit.
pub async fn write_queue(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<HealthKitWriteQueueRow>>, ApiError> {
    let rows = db::get_pending(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /healthkit/confirm — confirm that items have been written to HealthKit.
pub async fn confirm(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<HealthKitConfirm>,
) -> Result<StatusCode, ApiError> {
    db::confirm(&state.pool, user_id, &body.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
