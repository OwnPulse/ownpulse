// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::lab_results as db;
use crate::error::ApiError;
use crate::models::lab_result::{BulkCreateLabResults, CreateLabResult, LabResultQuery, LabResultRow};
use crate::routes::events::publish_event;

/// POST /labs
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateLabResult>,
) -> Result<(StatusCode, Json<LabResultRow>), ApiError> {
    let row = db::insert(&state.pool, user_id, &body).await?;
    publish_event(&state.event_tx, user_id, "labs", None);
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /labs
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<LabResultQuery>,
) -> Result<Json<Vec<LabResultRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.start, query.end).await?;
    Ok(Json(rows))
}

/// GET /labs/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<LabResultRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// POST /labs/bulk — bulk insert lab results (used by clinical records sync)
pub async fn bulk_create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<BulkCreateLabResults>,
) -> Result<(StatusCode, Json<Vec<LabResultRow>>), ApiError> {
    let rows = db::bulk_insert(&state.pool, user_id, &body.records).await?;
    if !rows.is_empty() {
        publish_event(&state.event_tx, user_id, "labs", None);
    }
    Ok((StatusCode::CREATED, Json(rows)))
}

/// DELETE /labs/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
