// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::models::invite::InviteRow;

/// Raw DB row for invite check queries (includes inviter info via JOIN).
#[derive(FromRow)]
pub struct InviteCheckRow {
    pub id: Uuid,
    pub label: Option<String>,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub inviter_name: Option<String>,
}

/// Raw DB row for invite claim queries.
#[derive(FromRow)]
pub struct ClaimRow {
    pub user_email: String,
    pub claimed_at: DateTime<Utc>,
}

/// Aggregated invite stats row.
#[derive(FromRow)]
pub struct InviteStatsRow {
    pub total: i64,
    pub active: i64,
    pub used: i64,
    pub expired: i64,
    pub revoked: i64,
}

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

/// Record that a user claimed an invite code. Runs inside the registration
/// transaction so the claim record is committed atomically with the user
/// creation and invite use_count increment.
pub async fn record_invite_claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invite_code_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO invite_claims (invite_code_id, user_id) VALUES ($1, $2)")
        .bind(invite_code_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
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

/// Look up an invite code by its code string, returning validity metadata.
/// JOINs with `users` to get the inviter's username/email.
pub async fn check_invite(
    pool: &PgPool,
    code: &str,
) -> Result<Option<InviteCheckRow>, sqlx::Error> {
    sqlx::query_as::<_, InviteCheckRow>(
        "SELECT ic.id, ic.label, ic.max_uses, ic.use_count, ic.expires_at,
                ic.revoked_at,
                COALESCE(u.username, u.email) AS inviter_name
         FROM invite_codes ic
         JOIN users u ON u.id = ic.created_by
         WHERE ic.code = $1",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
}

/// List all users who claimed a specific invite code, with masked emails.
pub async fn list_claims(
    pool: &PgPool,
    invite_code_id: Uuid,
) -> Result<Vec<ClaimRow>, sqlx::Error> {
    sqlx::query_as::<_, ClaimRow>(
        "SELECT u.email AS user_email, ic.claimed_at
         FROM invite_claims ic
         JOIN users u ON u.id = ic.user_id
         WHERE ic.invite_code_id = $1
         ORDER BY ic.claimed_at ASC",
    )
    .bind(invite_code_id)
    .fetch_all(pool)
    .await
}

/// Return aggregate invite code stats.
pub async fn invite_stats(pool: &PgPool) -> Result<InviteStatsRow, sqlx::Error> {
    sqlx::query_as::<_, InviteStatsRow>(
        "SELECT
            COUNT(*)::bigint AS total,
            COUNT(*) FILTER (
                WHERE revoked_at IS NULL
                  AND (expires_at IS NULL OR expires_at > now())
                  AND (max_uses IS NULL OR use_count < max_uses)
            )::bigint AS active,
            COUNT(*) FILTER (WHERE use_count > 0)::bigint AS used,
            COUNT(*) FILTER (
                WHERE expires_at IS NOT NULL AND expires_at <= now()
                  AND revoked_at IS NULL
            )::bigint AS expired,
            COUNT(*) FILTER (WHERE revoked_at IS NOT NULL)::bigint AS revoked
         FROM invite_codes",
    )
    .fetch_one(pool)
    .await
}
