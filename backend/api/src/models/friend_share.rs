// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
pub struct FriendShareRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub friend_id: Option<Uuid>,
    pub status: String,
    pub invite_token: Option<String>,
    pub invite_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct FriendShareResponse {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_email: String,
    pub friend_id: Option<Uuid>,
    pub friend_email: Option<String>,
    pub status: String,
    pub invite_token: Option<String>,
    pub data_types: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct CreateShareRequest {
    /// If provided, share directly with this user by email. If absent, generate invite link.
    pub friend_email: Option<String>,
    pub data_types: Vec<String>,
}

#[derive(Deserialize)]
pub struct AcceptLinkRequest {
    pub token: String,
}

#[derive(Deserialize)]
pub struct UpdatePermissionsRequest {
    pub data_types: Vec<String>,
}
