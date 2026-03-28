// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db;
use crate::db::health_records as db_hr;
use crate::db::healthkit as db_healthkit;
use crate::error::ApiError;
use crate::models::health_record::{CreateHealthRecord, HealthRecordQuery, HealthRecordRow};
use crate::routes::events::publish_event;

/// POST /health-records
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateHealthRecord>,
) -> Result<(StatusCode, Json<HealthRecordRow>), ApiError> {
    // Check for duplicates (within 60s and 2% value tolerance from a different source)
    let duplicate_of = match db_hr::find_duplicate(&state.pool, user_id, &body).await {
        Ok(Some(dup)) => {
            tracing::warn!(
                user_id = %user_id,
                existing_id = %dup.id,
                existing_source = %dup.source,
                new_source = %body.source,
                record_type = %body.record_type,
                "duplicate health record detected"
            );
            Some(dup.id)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %e, "failed to check for duplicate health record");
            None
        }
    };

    let row = db_hr::insert(&state.pool, user_id, &body, duplicate_of).await?;

    publish_event(&state.event_tx, user_id, "health_records", Some(&body.record_type));

    // If source is not healthkit, enqueue for HealthKit write-back (non-fatal)
    if body.source != "healthkit" {
        db_healthkit::enqueue_write(
            &state.pool,
            user_id,
            &row.record_type,
            &serde_json::json!({
                "value": row.value,
                "unit": row.unit,
                "start_time": row.start_time,
                "end_time": row.end_time,
            }),
            Some(row.id),
            Some("health_records"),
        )
        .await
        .ok();
    }

    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /health-records
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<HealthRecordQuery>,
) -> Result<Json<Vec<HealthRecordRow>>, ApiError> {
    let rows = db_hr::list(
        &state.pool,
        user_id,
        query.record_type.as_deref(),
        query.source.as_deref(),
        query.start,
        query.end,
    )
    .await?;
    Ok(Json(rows))
}

/// GET /health-records/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<HealthRecordRow>, ApiError> {
    let row = db_hr::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// DELETE /health-records/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db_hr::delete(&state.pool, user_id, id).await?;
    if deleted {
        // Fire-and-forget: audit log insert must not block or fail the response.
        let pool = state.pool.clone();
        tokio::spawn(async move {
            if let Err(e) =
                db::audit::log_access(&pool, user_id, "delete", "health_record", Some(id), None)
                    .await
            {
                tracing::warn!(error = %e, user_id = %user_id, record_id = %id, "audit log insert failed");
            }
        });
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
