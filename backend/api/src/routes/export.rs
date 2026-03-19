// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};

use crate::auth::extractor::AuthUser;
use crate::error::ApiError;
use crate::AppState;

/// GET /export/json — streaming JSON export of all user data.
pub async fn export_json(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Response, ApiError> {
    let body = crate::export::json::stream_json_export(&state.pool, user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok((
        [
            (CONTENT_TYPE, "application/json"),
            (
                CONTENT_DISPOSITION,
                "attachment; filename=\"ownpulse-export.json\"",
            ),
        ],
        body,
    )
        .into_response())
}

/// GET /export/csv — streaming CSV export of all user data.
pub async fn export_csv(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Response, ApiError> {
    let body = crate::export::csv::stream_csv_export(&state.pool, user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok((
        [
            (CONTENT_TYPE, "text/csv"),
            (
                CONTENT_DISPOSITION,
                "attachment; filename=\"ownpulse-export.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}
