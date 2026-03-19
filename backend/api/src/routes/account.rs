// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::auth::extractor::AuthUser;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::UserResponse;
use crate::AppState;

/// GET /account — return the current user's profile.
pub async fn get_account(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    let user = users::find_by_id(&state.pool, user_id).await?;
    Ok(Json(UserResponse::from(user)))
}

/// DELETE /account — permanently delete the user and all their data.
pub async fn delete_account(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<StatusCode, ApiError> {
    users::delete_user(&state.pool, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
