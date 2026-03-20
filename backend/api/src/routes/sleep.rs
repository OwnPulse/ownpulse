// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db::sleep as db;
use crate::error::ApiError;
use crate::models::sleep::{CreateSleep, SleepQuery, SleepRow};
use crate::AppState;

/// POST /sleep — create a new sleep record.
pub async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<CreateSleep>,
) -> Result<(StatusCode, Json<SleepRow>), ApiError> {
    let row = db::insert(&state.pool, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /sleep
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(query): Query<SleepQuery>,
) -> Result<Json<Vec<SleepRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.start, query.end).await?;
    Ok(Json(rows))
}

/// GET /sleep/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SleepRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// DELETE /sleep/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete(&state.pool, user_id, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
