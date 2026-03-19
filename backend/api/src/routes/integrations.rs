// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::auth::extractor::AuthUser;
use crate::db::integration_tokens as db;
use crate::error::ApiError;
use crate::AppState;

#[derive(Serialize)]
pub struct IntegrationStatus {
    pub source: String,
    pub connected: bool,
}

/// GET /integrations — list all integrations and their connection status.
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<IntegrationStatus>>, ApiError> {
    let tokens = db::list_for_user(&state.pool, user_id).await?;
    let statuses = tokens
        .into_iter()
        .map(|t| IntegrationStatus {
            source: t.source,
            connected: true,
        })
        .collect();
    Ok(Json(statuses))
}

/// DELETE /integrations/:source — disconnect an integration by removing its tokens.
pub async fn disconnect(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(source): Path<String>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, &source).await?;
    Ok(StatusCode::NO_CONTENT)
}
