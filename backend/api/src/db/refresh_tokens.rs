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
    pub family_id: Uuid,
}

/// Insert a new refresh token with a new family (initial login).
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    let family_id = Uuid::new_v4();
    insert_with_family(pool, user_id, token_hash, expires_at, family_id).await?;
    Ok(family_id)
}

/// Insert a new refresh token inheriting an existing family (rotation).
pub async fn insert_with_family(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
    family_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at, family_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(family_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Look up a refresh token by its hash.
pub async fn find_by_hash(pool: &PgPool, token_hash: &str) -> Result<RefreshTokenRow, sqlx::Error> {
    sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, created_at, family_id
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

/// Revoke all refresh tokens in a given family (replay detection).
pub async fn delete_family(pool: &PgPool, family_id: Uuid) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE family_id = $1")
        .bind(family_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
