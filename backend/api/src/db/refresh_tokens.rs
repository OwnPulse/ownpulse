// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct RefreshTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Insert a new refresh token.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Look up a refresh token by its hash.
pub async fn find_by_hash(pool: &PgPool, token_hash: &str) -> Result<RefreshTokenRow, sqlx::Error> {
    sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, created_at
         FROM refresh_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_one(pool)
    .await
}

/// Delete a single refresh token by its hash.
pub async fn delete_by_hash(pool: &PgPool, token_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revoke all refresh tokens for a user (e.g. on password change or logout-all).
pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
