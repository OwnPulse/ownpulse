// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AdminUser;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::UserResponse;

/// GET /admin/users — list all users (admin only).
pub async fn list_users(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> Result<Json<Vec<UserResponse>>, ApiError> {
    let rows = users::list_all_users(&state.pool).await?;
    Ok(Json(rows.into_iter().map(UserResponse::from).collect()))
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    pub role: String,
}

/// PATCH /admin/users/:id/role — change a user's role (admin only, can't change own).
pub async fn update_role(
    State(state): State<AppState>,
    AdminUser(admin_id): AdminUser,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if admin_id == user_id {
        return Err(ApiError::BadRequest(
            "cannot change your own role".to_string(),
        ));
    }
    if body.role != "admin" && body.role != "user" {
        return Err(ApiError::BadRequest(
            "role must be 'admin' or 'user'".to_string(),
        ));
    }
    let user = users::update_user_role(&state.pool, user_id, &body.role).await?;
    Ok(Json(UserResponse::from(user)))
}
