// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::Duration;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::models::friend_share::{
    AcceptLinkRequest, CreateShareRequest, FriendShareResponse, UpdatePermissionsRequest,
};

/// Mask an email for privacy: show first char + *** + domain.
/// e.g., "tony@gmail.com" → "t***@gmail.com"
fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() => {
            format!("{}***@{}", &local[..1], domain)
        }
        _ => "***".to_string(),
    }
}

const VALID_DATA_TYPES: &[&str] = &[
    "checkins",
    "health_records",
    "interventions",
    "observations",
    "lab_results",
];

fn validate_data_types(data_types: &[String]) -> Result<(), ApiError> {
    if data_types.is_empty() {
        return Err(ApiError::BadRequest(
            "data_types must not be empty".to_string(),
        ));
    }
    for dt in data_types {
        if !VALID_DATA_TYPES.contains(&dt.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "invalid data type: {dt}. Valid types: {}",
                VALID_DATA_TYPES.join(", ")
            )));
        }
    }
    Ok(())
}

/// POST /friends/shares — create a new share (direct or invite link).
pub async fn create_share(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateShareRequest>,
) -> Result<(StatusCode, Json<FriendShareResponse>), ApiError> {
    validate_data_types(&body.data_types)?;

    let (friend_id, invite_token, invite_expires_at) = if let Some(ref email) = body.friend_email {
        let friend = db::users::find_by_email(&state.pool, email).await?;
        if friend.id == user_id {
            return Err(ApiError::BadRequest(
                "cannot share with yourself".to_string(),
            ));
        }
        (Some(friend.id), None, None)
    } else {
        let token = Uuid::new_v4().to_string();
        let expires = chrono::Utc::now() + Duration::days(7);
        (None, Some(token), Some(expires))
    };

    let share = db::friend_shares::create_share(
        &state.pool,
        user_id,
        friend_id,
        invite_token.as_deref(),
        invite_expires_at,
    )
    .await?;

    db::friend_shares::set_permissions(&state.pool, share.id, &body.data_types).await?;

    // Build response
    let friend_email = body.friend_email.clone();

    let owner = db::users::find_by_id(&state.pool, user_id).await?;

    let response = FriendShareResponse {
        id: share.id,
        owner_id: share.owner_id,
        owner_email: owner.email,
        friend_id: share.friend_id,
        friend_email,
        status: share.status,
        invite_token: share.invite_token,
        data_types: body.data_types,
        created_at: share.created_at,
        accepted_at: share.accepted_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /friends/shares/outgoing — list shares I've created.
pub async fn list_outgoing(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<FriendShareResponse>>, ApiError> {
    let shares = db::friend_shares::list_outgoing(&state.pool, user_id).await?;
    Ok(Json(shares))
}

/// GET /friends/shares/incoming — list shares others have with me.
pub async fn list_incoming(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<FriendShareResponse>>, ApiError> {
    let mut responses = db::friend_shares::list_incoming(&state.pool, user_id).await?;
    // Mask owner email for non-accepted shares (prevent enumeration)
    for share in &mut responses {
        if share.status != "accepted" {
            share.owner_email = mask_email(&share.owner_email);
        }
    }
    Ok(Json(responses))
}

/// POST /friends/shares/:id/accept — accept a direct share.
pub async fn accept_share(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::friend_shares::accept_share(&state.pool, id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /friends/shares/accept-link — accept a share via invite token.
pub async fn accept_link(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<AcceptLinkRequest>,
) -> Result<Json<Value>, ApiError> {
    let share = db::friend_shares::accept_by_token(&state.pool, &body.token, user_id).await?;

    Ok(Json(serde_json::json!({
        "id": share.id,
        "owner_id": share.owner_id,
        "status": share.status,
        "accepted_at": share.accepted_at,
    })))
}

/// DELETE /friends/shares/:id — revoke or decline a share.
pub async fn revoke_share(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::friend_shares::revoke_share(&state.pool, id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// PATCH /friends/shares/:id/permissions — update data type permissions.
pub async fn update_permissions(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePermissionsRequest>,
) -> Result<StatusCode, ApiError> {
    validate_data_types(&body.data_types)?;

    let share = db::friend_shares::get_share(&state.pool, id).await?;
    if share.owner_id != user_id {
        return Err(ApiError::Forbidden);
    }

    db::friend_shares::set_permissions(&state.pool, id, &body.data_types).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /friends/:friend_id/data — get a friend's shared data.
pub async fn get_friend_data(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(friend_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    // friend_id is the data owner, user_id is the viewer
    let permitted_types =
        db::friend_shares::get_permitted_types(&state.pool, friend_id, user_id).await?;

    if permitted_types.is_empty() {
        return Err(ApiError::Forbidden);
    }

    let mut result = Map::new();

    for data_type in &permitted_types {
        match data_type.as_str() {
            "checkins" => {
                let items = db::checkins::list(&state.pool, friend_id, None, None).await?;
                result.insert(
                    "checkins".to_string(),
                    serde_json::to_value(items).map_err(|e| ApiError::Internal(e.to_string()))?,
                );
            }
            "health_records" => {
                let items =
                    db::health_records::list(&state.pool, friend_id, None, None, None, None)
                        .await?;
                result.insert(
                    "health_records".to_string(),
                    serde_json::to_value(items).map_err(|e| ApiError::Internal(e.to_string()))?,
                );
            }
            "interventions" => {
                let items = db::interventions::list(&state.pool, friend_id, None, None).await?;
                result.insert(
                    "interventions".to_string(),
                    serde_json::to_value(items).map_err(|e| ApiError::Internal(e.to_string()))?,
                );
            }
            "observations" => {
                let items = db::observations::list(&state.pool, friend_id, None).await?;
                result.insert(
                    "observations".to_string(),
                    serde_json::to_value(items).map_err(|e| ApiError::Internal(e.to_string()))?,
                );
            }
            "lab_results" => {
                let items = db::lab_results::list(&state.pool, friend_id, None, None).await?;
                result.insert(
                    "lab_results".to_string(),
                    serde_json::to_value(items).map_err(|e| ApiError::Internal(e.to_string()))?,
                );
            }
            _ => {}
        }
    }

    Ok(Json(Value::Object(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("tony@gmail.com"), "t***@gmail.com");
        assert_eq!(mask_email("a@example.com"), "a***@example.com");
        assert_eq!(mask_email("@broken.com"), "***");
        assert_eq!(mask_email("noatsign"), "***");
    }
}
