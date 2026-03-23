// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AdminUser;
use crate::db::{invites, refresh_tokens, users};
use crate::error::ApiError;
use crate::models::invite::{CreateInviteRequest, InviteResponse};
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

#[derive(Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

/// PATCH /admin/users/:id/status — enable or disable a user (admin only, can't change self).
pub async fn update_status(
    State(state): State<AppState>,
    AdminUser(admin_id): AdminUser,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateStatusRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if admin_id == user_id {
        return Err(ApiError::BadRequest(
            "cannot change your own status".to_string(),
        ));
    }
    if body.status != "active" && body.status != "disabled" {
        return Err(ApiError::BadRequest(
            "status must be 'active' or 'disabled'".to_string(),
        ));
    }
    let user = users::update_user_status(&state.pool, user_id, &body.status).await?;

    // When disabling a user, revoke all their refresh tokens so existing sessions
    // cannot be refreshed.
    if body.status == "disabled" {
        refresh_tokens::delete_all_for_user(&state.pool, user_id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    Ok(Json(UserResponse::from(user)))
}

/// DELETE /admin/users/:id — delete a user and all their data (admin only, can't delete self).
pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(admin_id): AdminUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    if admin_id == user_id {
        return Err(ApiError::BadRequest("cannot delete yourself".to_string()));
    }
    // Verify user exists and is disabled before attempting delete
    let target_user = users::find_by_id(&state.pool, user_id).await?;
    if target_user.status != "disabled" {
        return Err(ApiError::BadRequest(
            "user must be disabled before deletion".to_string(),
        ));
    }
    users::delete_user(&state.pool, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Generate a random 16-character base62 invite code.
fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// POST /admin/invites — create a new invite code (admin only).
pub async fn create_invite(
    State(state): State<AppState>,
    AdminUser(admin_id): AdminUser,
    Json(body): Json<CreateInviteRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), ApiError> {
    if let Some(max_uses) = body.max_uses
        && max_uses <= 0
    {
        return Err(ApiError::BadRequest(
            "max_uses must be greater than 0".to_string(),
        ));
    }
    if let Some(expires_in_hours) = body.expires_in_hours
        && expires_in_hours <= 0
    {
        return Err(ApiError::BadRequest(
            "expires_in_hours must be greater than 0".to_string(),
        ));
    }

    let code = generate_invite_code();

    let expires_at = body
        .expires_in_hours
        .map(|hours| Utc::now() + Duration::hours(hours));

    let row = invites::create_invite(
        &state.pool,
        admin_id,
        &code,
        body.label.as_deref(),
        body.max_uses,
        expires_at,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(InviteResponse::from(row))))
}

/// GET /admin/invites — list all invite codes (admin only).
pub async fn list_invites(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> Result<Json<Vec<InviteResponse>>, ApiError> {
    let rows = invites::list_invites(&state.pool).await?;
    Ok(Json(rows.into_iter().map(InviteResponse::from).collect()))
}

/// DELETE /admin/invites/:id — revoke an invite code (admin only).
pub async fn revoke_invite(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(invite_id): Path<Uuid>,
) -> Result<Json<InviteResponse>, ApiError> {
    let row = invites::revoke_invite(&state.pool, invite_id).await?;
    Ok(Json(InviteResponse::from(row)))
}
