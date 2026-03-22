// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::invite::InviteRow;

/// Create a new invite code.
pub async fn create_invite(
    pool: &PgPool,
    created_by: Uuid,
    code: &str,
    label: Option<&str>,
    max_uses: Option<i32>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<InviteRow, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "INSERT INTO invite_codes (created_by, code, label, max_uses, expires_at)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, code, created_by, label, max_uses, use_count,
                   expires_at, revoked_at, created_at",
    )
    .bind(created_by)
    .bind(code)
    .bind(label)
    .bind(max_uses)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

/// Atomically claim an invite code: check validity and increment use_count in one query.
///
/// Returns Ok(InviteRow) with the updated row if the code was successfully claimed,
/// or Err(sqlx::Error::RowNotFound) if the code is invalid, expired, revoked, or maxed out.
/// This prevents TOCTOU race conditions by combining the check and update in a single statement.
pub async fn claim_invite_code_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: &str,
) -> Result<InviteRow, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "UPDATE invite_codes
         SET use_count = use_count + 1
         WHERE code = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())
           AND (max_uses IS NULL OR use_count < max_uses)
         RETURNING id, code, created_by, label, max_uses, use_count,
                   expires_at, revoked_at, created_at",
    )
    .bind(code)
    .fetch_one(&mut **tx)
    .await
}

/// List all invite codes, ordered by creation date (newest first).
pub async fn list_invites(pool: &PgPool) -> Result<Vec<InviteRow>, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "SELECT id, code, created_by, label, max_uses, use_count,
                expires_at, revoked_at, created_at
         FROM invite_codes ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Revoke an invite code by setting `revoked_at`.
pub async fn revoke_invite(pool: &PgPool, invite_id: Uuid) -> Result<InviteRow, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "UPDATE invite_codes SET revoked_at = now() WHERE id = $1
         RETURNING id, code, created_by, label, max_uses, use_count,
                   expires_at, revoked_at, created_at",
    )
    .bind(invite_id)
    .fetch_one(pool)
    .await
}
