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
    /// When the token was rotated out. NULL means the token is active; a
    /// rotated token stays presentable for a short grace window so
    /// concurrent refreshes (multiple web tabs) don't 401 each other.
    pub rotated_at: Option<DateTime<Utc>>,
    /// The successor token, AES-256-GCM encrypted. Set at rotation so
    /// within-grace presentations return the same successor instead of
    /// minting a fork; deleted with the row when the family is swept.
    pub successor_ciphertext: Option<String>,
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
        "SELECT id, user_id, token_hash, expires_at, created_at, family_id, rotated_at,
                successor_ciphertext
         FROM refresh_tokens WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_one(pool)
    .await
}

/// Look up a refresh token by its hash and row-lock it for the transaction.
/// The lock serializes concurrent refreshes of the same token and makes a
/// concurrent `delete_family` (logout) wait until the rotation commits.
pub async fn find_by_hash_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_hash: &str,
) -> Result<Option<RefreshTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, created_at, family_id, rotated_at,
                successor_ciphertext
         FROM refresh_tokens WHERE token_hash = $1 FOR UPDATE",
    )
    .bind(token_hash)
    .fetch_optional(&mut **tx)
    .await
}

/// Mark a token as rotated and record its encrypted successor, inside the
/// caller's transaction.
pub async fn mark_rotated_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token_hash: &str,
    successor_ciphertext: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE refresh_tokens SET rotated_at = now(), successor_ciphertext = $2
         WHERE token_hash = $1",
    )
    .bind(token_hash)
    .bind(successor_ciphertext)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Insert a rotation successor inside the caller's transaction.
pub async fn insert_with_family_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Delete a family's dead tokens: rotated longer than `grace_seconds` ago,
/// or past expiry. Runs opportunistically on each rotation so nothing
/// accumulates for active families.
pub async fn cleanup_family(
    pool: &PgPool,
    family_id: Uuid,
    grace_seconds: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM refresh_tokens
         WHERE family_id = $1
           AND (rotated_at < now() - ($2 * interval '1 second')
                OR expires_at < now())",
    )
    .bind(family_id)
    .bind(grace_seconds)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Delete refresh tokens expired more than seven days ago. Rotation's
/// per-family sweep only covers families that rotate again; rows of
/// abandoned families otherwise accumulate until this global sweep removes
/// them. The seven-day margin keeps recently-expired rotated rows around so
/// presenting one still fires the post-grace theft detection instead of the
/// benign unknown-token path.
pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now() - interval '7 days'")
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

/// Revoke a family inside the caller's transaction. The refresh handler
/// holds the presented row's lock, so a pool-side revocation would deadlock
/// against itself; the post-commit [`delete_family`] passes catch any
/// successor a concurrent rotation slipped past this DELETE's snapshot.
pub async fn delete_family_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    family_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE family_id = $1")
        .bind(family_id)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected())
}

/// Revocation DELETEs re-run until a pass removes nothing. A rotation
/// committing mid-revocation inserts a successor the first DELETE's scan
/// cannot see (its snapshot predates the row); the next pass catches it.
/// Each escape needs a full refresh round-trip racing a back-to-back DELETE,
/// so the loop converges immediately in practice.
const REVOKE_MAX_PASSES: u32 = 5;

/// Revoke all refresh tokens for a user (e.g. on password change or logout-all).
pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    for _ in 0..REVOKE_MAX_PASSES {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(());
        }
    }
    tracing::warn!(%user_id, "refresh token revocation still deleting after max passes");
    Ok(())
}

/// Revoke all refresh tokens in a given family (logout, reuse detection).
pub async fn delete_family(pool: &PgPool, family_id: Uuid) -> Result<u64, sqlx::Error> {
    let mut total = 0;
    for _ in 0..REVOKE_MAX_PASSES {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE family_id = $1")
            .bind(family_id)
            .execute(pool)
            .await?;
        total += result.rows_affected();
        if result.rows_affected() == 0 {
            return Ok(total);
        }
    }
    tracing::warn!(%family_id, "refresh token revocation still deleting after max passes");
    Ok(total)
}
