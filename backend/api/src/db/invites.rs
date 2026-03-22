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
         RETURNING *",
    )
    .bind(created_by)
    .bind(code)
    .bind(label)
    .bind(max_uses)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

/// Find a valid (non-revoked, non-expired, not at max uses) invite code.
pub async fn find_valid_code(pool: &PgPool, code: &str) -> Result<Option<InviteRow>, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "SELECT * FROM invite_codes
         WHERE code = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())
           AND (max_uses IS NULL OR use_count < max_uses)",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
}

/// Atomically increment the use count of an invite code.
/// Returns the updated row. Fails if the code is no longer valid.
pub async fn claim_invite_code_tx(
    pool: &PgPool,
    invite_id: Uuid,
) -> Result<InviteRow, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "UPDATE invite_codes
         SET use_count = use_count + 1
         WHERE id = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())
           AND (max_uses IS NULL OR use_count < max_uses)
         RETURNING *",
    )
    .bind(invite_id)
    .fetch_one(pool)
    .await
}

/// List all invite codes ordered by creation date.
pub async fn list_invites(pool: &PgPool) -> Result<Vec<InviteRow>, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>("SELECT * FROM invite_codes ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

/// Revoke an invite code by setting its `revoked_at` timestamp.
pub async fn revoke_invite(pool: &PgPool, invite_id: Uuid) -> Result<InviteRow, sqlx::Error> {
    sqlx::query_as::<_, InviteRow>(
        "UPDATE invite_codes SET revoked_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(invite_id)
    .fetch_one(pool)
    .await
}
