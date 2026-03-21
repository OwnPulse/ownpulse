// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Audit log database access.
//!
//! Provides a single insert function used by handlers to record sensitive
//! operations (exports, deletes, account deletion). Callers fire-and-forget
//! via `tokio::spawn` so the audit write never blocks the response path.

use sqlx::PgPool;
use uuid::Uuid;

/// Record a sensitive data operation in `data_access_log`.
///
/// Intended to be called inside `tokio::spawn`; errors are non-fatal and
/// should be logged by the caller rather than propagated.
pub async fn log_access(
    pool: &PgPool,
    user_id: Uuid,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    ip_address: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO data_access_log \
         (user_id, action, resource_type, resource_id, ip_address) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(ip_address)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return up to 100 audit log entries for a user, newest first.
pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<AuditEntry>, sqlx::Error> {
    sqlx::query_as::<_, AuditEntry>(
        "SELECT id, user_id, action, resource_type, resource_id, ip_address, created_at \
         FROM data_access_log \
         WHERE user_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// A single row from `data_access_log`, returned to the API caller.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Uuid,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
