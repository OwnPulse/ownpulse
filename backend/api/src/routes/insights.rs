// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::insights as db;
use crate::error::ApiError;
use crate::jobs::insight_generator;
use crate::models::insight::InsightRow;

/// GET /insights — list active (non-dismissed) insights, newest first, limit 10.
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<InsightRow>>, ApiError> {
    let rows = db::list_active(&state.pool, user_id, 10).await?;
    Ok(Json(rows))
}

/// POST /insights/:id/dismiss — mark an insight as dismissed.
pub async fn dismiss(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let updated = db::dismiss(&state.pool, user_id, id).await?;
    if updated {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

/// POST /insights/generate — trigger insight generation for the current user.
pub async fn generate(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<(StatusCode, Json<Vec<InsightRow>>), ApiError> {
    let insights = insight_generator::generate_for_user(&state.pool, user_id).await?;
    Ok((StatusCode::OK, Json(insights)))
}
