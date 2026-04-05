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
use crate::db::{feature_flags, invites, protocols, refresh_tokens, users};
use crate::email;
use crate::error::ApiError;
use crate::models::feature_flag::{FeatureFlagResponse, UpsertFlagRequest};
use crate::models::invite::{
    CreateInviteRequest, InviteCheckResponse, InviteClaimResponse, InviteResponse,
    InviteStatsResponse, SendInviteEmailRequest,
};
use crate::models::protocol::{AdminBulkImportRequest, PromoteRequest, ProtocolExport};
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

    // Send invite email if requested
    if let Some(ref recipient) = body.send_to_email {
        let admin_user = users::find_by_id(&state.pool, admin_id).await?;
        let inviter = if admin_user.email.is_empty() {
            "Someone"
        } else {
            &admin_user.email
        };
        let url = format!("{}/invite/{}", state.config.web_origin, code);
        let expiry = expires_at
            .map(|e| format!("This invite expires on {}.", e.format("%B %d, %Y")))
            .unwrap_or_default();
        let html = invite_email_html(inviter, &url, &expiry);
        if let Err(e) = email::send_email(
            &state.config,
            recipient,
            &format!("{inviter} invited you to OwnPulse"),
            &html,
        )
        .await
        {
            tracing::warn!(error = %e, to = %recipient, "failed to send invite email");
        }
    }

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

/// GET /invites/:code/check — public endpoint to validate an invite code.
pub async fn check_invite(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<InviteCheckResponse>, ApiError> {
    let row = invites::check_invite(&state.pool, &code).await?;

    let response = match row {
        None => InviteCheckResponse {
            valid: false,
            label: None,
            expires_at: None,
            inviter_name: None,
            reason: Some("not_found".to_string()),
        },
        Some(r) if r.revoked_at.is_some() => InviteCheckResponse {
            valid: false,
            label: None,
            expires_at: None,
            inviter_name: None,
            reason: Some("revoked".to_string()),
        },
        Some(r) if r.expires_at.is_some() && r.expires_at.unwrap() < Utc::now() => {
            InviteCheckResponse {
                valid: false,
                label: None,
                expires_at: None,
                inviter_name: None,
                reason: Some("expired".to_string()),
            }
        }
        Some(r) if r.max_uses.is_some() && r.use_count >= r.max_uses.unwrap() => {
            InviteCheckResponse {
                valid: false,
                label: None,
                expires_at: None,
                inviter_name: None,
                reason: Some("exhausted".to_string()),
            }
        }
        Some(r) => InviteCheckResponse {
            valid: true,
            label: r.label,
            expires_at: r.expires_at,
            inviter_name: r.inviter_name,
            reason: None,
        },
    };

    Ok(Json(response))
}

/// Mask an email address: "tony@example.com" → "t***@example.com".
fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() => {
            format!("{}***@{}", &local[..1], domain)
        }
        _ => "***".to_string(),
    }
}

/// GET /admin/invites/:id/claims — list users who claimed an invite code (admin only).
pub async fn invite_claims(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(invite_id): Path<Uuid>,
) -> Result<Json<Vec<InviteClaimResponse>>, ApiError> {
    let rows = invites::list_claims(&state.pool, invite_id).await?;
    let claims = rows
        .into_iter()
        .map(|r| InviteClaimResponse {
            user_email: mask_email(&r.user_email),
            claimed_at: r.claimed_at,
        })
        .collect();
    Ok(Json(claims))
}

/// GET /admin/invites/stats — invite summary stats (admin only).
pub async fn invite_stats(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> Result<Json<InviteStatsResponse>, ApiError> {
    let row = invites::invite_stats(&state.pool).await?;
    Ok(Json(InviteStatsResponse {
        total: row.total,
        active: row.active,
        used: row.used,
        expired: row.expired,
        revoked: row.revoked,
    }))
}

/// POST /admin/invites/:id/send-email — send/resend an invite email.
pub async fn send_invite_email(
    State(state): State<AppState>,
    AdminUser(admin_id): AdminUser,
    Path(invite_id): Path<Uuid>,
    Json(body): Json<SendInviteEmailRequest>,
) -> Result<StatusCode, ApiError> {
    let rows = invites::list_invites(&state.pool).await?;
    let invite = rows
        .into_iter()
        .find(|r| r.id == invite_id)
        .ok_or(ApiError::NotFound)?;

    if invite.revoked_at.is_some() {
        return Err(ApiError::BadRequest("invite is revoked".to_string()));
    }

    let admin_user = users::find_by_id(&state.pool, admin_id).await?;
    let inviter = if admin_user.email.is_empty() {
        "Someone"
    } else {
        &admin_user.email
    };
    let url = format!("{}/invite/{}", state.config.web_origin, invite.code);
    let expiry = invite
        .expires_at
        .map(|e| format!("This invite expires on {}.", e.format("%B %d, %Y")))
        .unwrap_or_default();
    let html = invite_email_html(inviter, &url, &expiry);

    email::send_email(
        &state.config,
        &body.email,
        &format!("{inviter} invited you to OwnPulse"),
        &html,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("failed to send email: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /admin/protocols/:id/promote
pub async fn promote_protocol(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PromoteRequest>,
) -> Result<StatusCode, ApiError> {
    let promoted = protocols::promote_to_template(&state.pool, id, body.tags).await?;
    if !promoted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /admin/protocols/:id/demote
pub async fn demote_protocol(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let demoted = protocols::demote_template(&state.pool, id).await?;
    if !demoted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /admin/protocols/import
pub async fn admin_bulk_import(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Json(body): Json<AdminBulkImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (exports, source_url): (Vec<ProtocolExport>, Option<String>) = if let Some(url) = body.url {
        let resp = state
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiError::BadRequest(format!("failed to fetch URL: {e}")))?;

        if !resp.status().is_success() {
            return Err(ApiError::BadRequest(format!(
                "URL returned status {}",
                resp.status()
            )));
        }

        let data: Vec<ProtocolExport> = resp
            .json()
            .await
            .map_err(|e| ApiError::BadRequest(format!("invalid JSON from URL: {e}")))?;

        (data, Some(url))
    } else if let Some(data) = body.protocols {
        (data, None)
    } else {
        return Err(ApiError::BadRequest(
            "either url or protocols must be provided".to_string(),
        ));
    };

    // Validate all exports
    for export in &exports {
        if export.schema != "ownpulse-protocol/v1" {
            return Err(ApiError::BadRequest(format!(
                "unsupported schema '{}' in protocol '{}'",
                export.schema, export.name
            )));
        }
        if export.name.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "protocol name must not be empty".to_string(),
            ));
        }
    }

    let count =
        protocols::bulk_import_templates(&state.pool, &exports, source_url.as_deref()).await?;

    Ok(Json(serde_json::json!({ "imported": count })))
}

// ─── Feature Flags ────────────────────────────────────────────────────────────

/// GET /admin/feature-flags — list all feature flags (admin only).
pub async fn list_feature_flags(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> Result<Json<Vec<FeatureFlagResponse>>, ApiError> {
    let rows = feature_flags::list(&state.pool).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| FeatureFlagResponse {
                id: r.id,
                key: r.key,
                enabled: r.enabled,
                description: r.description,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect(),
    ))
}

/// PUT /admin/feature-flags/:key — create or update a feature flag (admin only).
pub async fn upsert_feature_flag(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(key): Path<String>,
    Json(body): Json<UpsertFlagRequest>,
) -> Result<Json<FeatureFlagResponse>, ApiError> {
    if key.is_empty() {
        return Err(ApiError::BadRequest(
            "feature flag key must not be empty".to_string(),
        ));
    }
    let row =
        feature_flags::upsert(&state.pool, &key, body.enabled, body.description.as_deref())
            .await?;
    Ok(Json(FeatureFlagResponse {
        id: row.id,
        key: row.key,
        enabled: row.enabled,
        description: row.description,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

/// DELETE /admin/feature-flags/:key — delete a feature flag (admin only).
pub async fn delete_feature_flag(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    feature_flags::delete(&state.pool, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn invite_email_html(inviter: &str, invite_url: &str, expiry_line: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#FAF6F1;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
<table width="100%" cellpadding="0" cellspacing="0" style="max-width:560px;margin:40px auto;background:#fff;border-radius:12px;overflow:hidden;box-shadow:0 2px 12px rgba(0,0,0,0.08);">
<tr><td style="background:#C2654A;padding:32px 40px;">
  <h1 style="margin:0;color:#fff;font-size:24px;font-weight:700;">OwnPulse</h1>
</td></tr>
<tr><td style="padding:40px;">
  <h2 style="margin:0 0 16px;color:#1a1a1a;font-size:22px;">You're invited!</h2>
  <p style="color:#444;font-size:16px;line-height:1.6;">
    {inviter} has invited you to join <strong>OwnPulse</strong> — a personal health data platform that puts you in control of your health data.
  </p>
  <p style="color:#444;font-size:16px;line-height:1.6;">
    Track health metrics from wearables, log daily check-ins, upload genetic data, and explore correlations.
  </p>
  <div style="text-align:center;margin:32px 0;">
    <a href="{invite_url}" style="display:inline-block;background:#C2654A;color:#fff;text-decoration:none;padding:14px 32px;border-radius:8px;font-size:16px;font-weight:600;">Create Your Account</a>
  </div>
  <p style="color:#888;font-size:14px;">{expiry_line}</p>
  <p style="color:#888;font-size:13px;margin-top:24px;">
    If the button doesn't work, copy this link:<br>
    <a href="{invite_url}" style="color:#C2654A;word-break:break-all;">{invite_url}</a>
  </p>
</td></tr>
<tr><td style="padding:20px 40px;background:#f9f5f0;border-top:1px solid #eee;">
  <p style="margin:0;color:#999;font-size:12px;">OwnPulse — Your data, your control.</p>
</td></tr>
</table></body></html>"#
    )
}
