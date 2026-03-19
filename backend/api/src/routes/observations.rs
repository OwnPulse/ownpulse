// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db::observations as db;
use crate::error::ApiError;
use crate::models::observation::{
    is_valid_observation_type, CreateObservation, ObservationQuery, ObservationRow,
};
use crate::AppState;

/// POST /observations — validates observation type before insert.
pub async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<CreateObservation>,
) -> Result<(StatusCode, Json<ObservationRow>), ApiError> {
    if !is_valid_observation_type(&body.obs_type) {
        return Err(ApiError::BadRequest(format!(
            "invalid observation type: {}",
            body.obs_type
        )));
    }

    let row = db::insert(&state.pool, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /observations
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(query): Query<ObservationQuery>,
) -> Result<Json<Vec<ObservationRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.obs_type.as_deref()).await?;
    Ok(Json(rows))
}

/// GET /observations/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ObservationRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// DELETE /observations/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
