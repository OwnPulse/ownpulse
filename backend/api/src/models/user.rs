// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub username: Option<String>,
    pub password_hash: Option<String>,
    pub auth_provider: String,
    pub email: String,
    pub role: String,
    pub data_region: String,
    pub federation_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: Option<String>,
    pub auth_provider: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub data_region: String,
    pub created_at: DateTime<Utc>,
}

impl From<UserRow> for UserResponse {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            auth_provider: row.auth_provider,
            email: row.email,
            role: row.role,
            status: row.status,
            data_region: row.data_region,
            created_at: row.created_at,
        }
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

/// Request body for `/auth/refresh` — iOS sends the refresh token in the body
/// instead of an httpOnly cookie.
#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// A single linked auth method returned to the client.
#[derive(Serialize, sqlx::FromRow)]
pub struct AuthMethodRow {
    pub id: Uuid,
    pub provider: String,
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Request body for `POST /auth/apple/callback`.
#[derive(Deserialize)]
pub struct AppleCallbackRequest {
    pub id_token: String,
    pub platform: String,
}

/// Request body for `POST /auth/link`.
#[derive(Deserialize)]
pub struct LinkAuthRequest {
    pub provider: String,
    pub id_token: Option<String>,
    pub password: Option<String>,
}

/// Token response that includes the refresh token in the JSON body.
///
/// Used for iOS clients that store tokens in the Keychain rather than
/// relying on httpOnly cookies.
#[derive(Serialize)]
pub struct TokenResponseWithRefresh {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}
