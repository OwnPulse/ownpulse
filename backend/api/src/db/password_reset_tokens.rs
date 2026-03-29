// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct PasswordResetTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Insert a new password reset token.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Find a valid (unclaimed, unexpired) token by its hash.
pub async fn find_valid_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<PasswordResetTokenRow, sqlx::Error> {
    sqlx::query_as::<_, PasswordResetTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, claimed_at, created_at
         FROM password_reset_tokens
         WHERE token_hash = $1 AND claimed_at IS NULL AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_one(pool)
    .await
}

/// Mark a token as claimed within a transaction.
pub async fn mark_claimed_tx(
    tx: &mut Transaction<'_, Postgres>,
    token_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE password_reset_tokens SET claimed_at = now() WHERE id = $1")
        .bind(token_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Invalidate all unclaimed tokens for a user (cancel previous reset requests).
pub async fn invalidate_all_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE password_reset_tokens SET claimed_at = now()
         WHERE user_id = $1 AND claimed_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
