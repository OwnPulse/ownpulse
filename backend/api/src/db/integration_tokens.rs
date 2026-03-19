// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct IntegrationTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Summary of integration connection status (no secrets).
#[derive(Debug, Serialize)]
pub struct IntegrationStatus {
    pub source: String,
    pub connected: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_sync_error: Option<String>,
}

/// List all integration tokens for a user.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<IntegrationTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, IntegrationTokenRow>(
        "SELECT id, user_id, source, access_token, refresh_token,
                expires_at, last_synced_at, last_sync_error, updated_at
         FROM integration_tokens
         WHERE user_id = $1
         ORDER BY source",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Upsert an integration token. On conflict (user_id, source), update all token
/// fields and reset sync error.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    source: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<IntegrationTokenRow, sqlx::Error> {
    sqlx::query_as::<_, IntegrationTokenRow>(
        "INSERT INTO integration_tokens
            (user_id, source, access_token, refresh_token, expires_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, source) DO UPDATE SET
            access_token   = EXCLUDED.access_token,
            refresh_token  = EXCLUDED.refresh_token,
            expires_at     = EXCLUDED.expires_at,
            last_sync_error = NULL,
            updated_at     = now()
         RETURNING id, user_id, source, access_token, refresh_token,
                   expires_at, last_synced_at, last_sync_error, updated_at",
    )
    .bind(user_id)
    .bind(source)
    .bind(access_token)
    .bind(refresh_token)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

/// Delete an integration token (disconnect a source).
pub async fn delete(
    pool: &PgPool,
    user_id: Uuid,
    source: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM integration_tokens WHERE user_id = $1 AND source = $2",
    )
    .bind(user_id)
    .bind(source)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
