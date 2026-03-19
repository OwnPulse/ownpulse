// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::auth::extractor::AuthUser;
use crate::db::source_preferences as db;
use crate::error::ApiError;
use crate::models::source_preference::{SourcePreferenceRow, UpsertSourcePreference};
use crate::AppState;

/// GET /source-preferences
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<SourcePreferenceRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /source-preferences — upsert a per-metric source preference.
pub async fn upsert(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<UpsertSourcePreference>,
) -> Result<(StatusCode, Json<SourcePreferenceRow>), ApiError> {
    let row = db::upsert(&state.pool, user_id, &body.metric_type, &body.preferred_source).await?;
    Ok((StatusCode::CREATED, Json(row)))
}
