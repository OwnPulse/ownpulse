// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto;

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

/// Decrypt the token fields of a single row in place.
fn decrypt_row(
    row: &mut IntegrationTokenRow,
    key: &[u8; 32],
    previous_key: Option<&[u8; 32]>,
) -> Result<(), crypto::CryptoError> {
    row.access_token = crypto::decrypt(&row.access_token, key, previous_key)?;
    if let Some(ref rt) = row.refresh_token {
        row.refresh_token = Some(crypto::decrypt(rt, key, previous_key)?);
    }
    Ok(())
}

/// List all integration tokens for a user, decrypting token fields.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    encryption_key: &[u8; 32],
    previous_key: Option<&[u8; 32]>,
) -> Result<Vec<IntegrationTokenRow>, sqlx::Error> {
    let mut rows = sqlx::query_as::<_, IntegrationTokenRow>(
        "SELECT id, user_id, source, access_token, refresh_token,
                expires_at, last_synced_at, last_sync_error, updated_at
         FROM integration_tokens
         WHERE user_id = $1
         ORDER BY source",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    for row in &mut rows {
        decrypt_row(row, encryption_key, previous_key).map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, source = %row.source, "failed to decrypt integration token");
            sqlx::Error::Protocol(format!("token decryption failed: {e}"))
        })?;
    }

    Ok(rows)
}

/// Upsert an integration token. Encrypts access_token and refresh_token before
/// storage. On conflict (user_id, source), update all token fields and reset
/// sync error.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    source: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
    encryption_key: &[u8; 32],
) -> Result<IntegrationTokenRow, sqlx::Error> {
    let encrypted_access = crypto::encrypt(access_token, encryption_key).map_err(|e| {
        tracing::error!(error = %e, "failed to encrypt access token");
        sqlx::Error::Protocol(format!("token encryption failed: {e}"))
    })?;

    let encrypted_refresh = refresh_token
        .map(|rt| {
            crypto::encrypt(rt, encryption_key).map_err(|e| {
                tracing::error!(error = %e, "failed to encrypt refresh token");
                sqlx::Error::Protocol(format!("token encryption failed: {e}"))
            })
        })
        .transpose()?;

    let mut row = sqlx::query_as::<_, IntegrationTokenRow>(
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
    .bind(&encrypted_access)
    .bind(encrypted_refresh.as_deref())
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    // Return the row with decrypted tokens so callers get plaintext back.
    row.access_token = access_token.to_string();
    row.refresh_token = refresh_token.map(|s| s.to_string());

    Ok(row)
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
