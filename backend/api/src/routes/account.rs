// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::UserResponse;

/// GET /account — return the current user's profile.
pub async fn get_account(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    let user = users::find_by_id(&state.pool, user_id).await?;
    Ok(Json(UserResponse::from(user)))
}

/// DELETE /account — permanently delete the user and all their data.
pub async fn delete_account(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<StatusCode, ApiError> {
    // Write the audit entry synchronously before deleting the user so that the
    // record is committed regardless of whether the caller's connection closes
    // immediately after. The data_access_log table has no FK on user_id
    // intentionally — the log must survive account deletion.
    if let Err(e) = db::audit::log_access(
        &state.pool,
        user_id,
        "delete_account",
        "account",
        None,
        None,
    )
    .await
    {
        tracing::warn!(error = %e, user_id = %user_id, "audit log insert failed before account deletion");
    }

    users::delete_user(&state.pool, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
