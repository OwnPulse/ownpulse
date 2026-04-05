// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Database row for the `feature_flags` table.
#[derive(Debug, FromRow)]
pub struct FeatureFlagRow {
    pub id: Uuid,
    pub key: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Return all feature flags as a key/enabled map.
pub async fn all_flags(pool: &PgPool) -> Result<HashMap<String, bool>, sqlx::Error> {
    let rows = sqlx::query_as::<_, FeatureFlagRow>(
        "SELECT id, key, enabled, description, created_at, updated_at FROM feature_flags",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| (r.key, r.enabled)).collect())
}

/// List all flags with full details, ordered by key.
pub async fn list(pool: &PgPool) -> Result<Vec<FeatureFlagRow>, sqlx::Error> {
    sqlx::query_as::<_, FeatureFlagRow>(
        "SELECT id, key, enabled, description, created_at, updated_at
         FROM feature_flags ORDER BY key",
    )
    .fetch_all(pool)
    .await
}

/// Insert or update a feature flag by key.
pub async fn upsert(
    pool: &PgPool,
    key: &str,
    enabled: bool,
    description: Option<&str>,
) -> Result<FeatureFlagRow, sqlx::Error> {
    sqlx::query_as::<_, FeatureFlagRow>(
        "INSERT INTO feature_flags (key, enabled, description)
         VALUES ($1, $2, $3)
         ON CONFLICT (key) DO UPDATE
           SET enabled = EXCLUDED.enabled,
               description = EXCLUDED.description,
               updated_at = now()
         RETURNING id, key, enabled, description, created_at, updated_at",
    )
    .bind(key)
    .bind(enabled)
    .bind(description)
    .fetch_one(pool)
    .await
}

/// Delete a feature flag by key. Returns an error if the key does not exist.
pub async fn delete(pool: &PgPool, key: &str) -> Result<(), sqlx::Error> {
    let result = sqlx::query("DELETE FROM feature_flags WHERE key = $1")
        .bind(key)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}
