// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::saved_medicines as db;
use crate::error::ApiError;
use crate::models::saved_medicine::{CreateSavedMedicine, SavedMedicineRow, UpdateSavedMedicine};

/// GET /saved-medicines
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<SavedMedicineRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /saved-medicines
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateSavedMedicine>,
) -> Result<(StatusCode, Json<SavedMedicineRow>), ApiError> {
    if body.substance.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "substance must not be empty".to_string(),
        ));
    }
    let row = db::insert(&state.pool, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// PUT /saved-medicines/:id
pub async fn update(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSavedMedicine>,
) -> Result<Json<SavedMedicineRow>, ApiError> {
    let row = db::update(&state.pool, user_id, id, &body)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(row))
}

/// DELETE /saved-medicines/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete(&state.pool, user_id, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
