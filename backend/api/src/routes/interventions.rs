// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::interventions as db;
use crate::error::ApiError;
use crate::models::intervention::{CreateIntervention, InterventionQuery, InterventionRow};
use crate::routes::events::publish_event;

/// POST /interventions — no substance name validation per project rules.
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateIntervention>,
) -> Result<(StatusCode, Json<InterventionRow>), ApiError> {
    let row = db::insert(&state.pool, user_id, &body).await?;
    publish_event(&state.event_tx, user_id, "interventions", None);
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /interventions
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<InterventionQuery>,
) -> Result<Json<Vec<InterventionRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.start, query.end).await?;
    Ok(Json(rows))
}

/// GET /interventions/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<InterventionRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// DELETE /interventions/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
