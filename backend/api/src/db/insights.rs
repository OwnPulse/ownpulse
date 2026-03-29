// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::insight::InsightRow;
use sqlx::PgPool;
use uuid::Uuid;

/// List active (non-dismissed) insights for a user, newest first, up to `limit`.
pub async fn list_active(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    sqlx::query_as::<_, InsightRow>(
        "SELECT id, user_id, insight_type, headline, detail, metadata,
                dismissed_at, created_at
         FROM insights
         WHERE user_id = $1 AND dismissed_at IS NULL
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Insert a new insight and return it.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    insight_type: &str,
    headline: &str,
    detail: Option<&str>,
    metadata: &serde_json::Value,
) -> Result<InsightRow, sqlx::Error> {
    sqlx::query_as::<_, InsightRow>(
        "INSERT INTO insights (user_id, insight_type, headline, detail, metadata)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, insight_type, headline, detail, metadata,
                   dismissed_at, created_at",
    )
    .bind(user_id)
    .bind(insight_type)
    .bind(headline)
    .bind(detail)
    .bind(metadata)
    .fetch_one(pool)
    .await
}

/// Dismiss an insight by setting `dismissed_at`. Returns true if a row was updated.
/// The `user_id` in the WHERE clause prevents IDOR — users can only dismiss their own.
pub async fn dismiss(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE insights SET dismissed_at = now()
         WHERE id = $1 AND user_id = $2 AND dismissed_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Check if a recent insight of the given type with a matching metadata key/value
/// exists within the last `days` days for the user. Used for deduplication.
pub async fn exists_recent(
    pool: &PgPool,
    user_id: Uuid,
    insight_type: &str,
    metadata_key: &str,
    metadata_value: &str,
    days: i32,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM insights
            WHERE user_id = $1
              AND insight_type = $2
              AND metadata->>$3 = $4
              AND created_at >= now() - make_interval(days => $5)
        )",
    )
    .bind(user_id)
    .bind(insight_type)
    .bind(metadata_key)
    .bind(metadata_value)
    .bind(days)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Delete insights older than `max_age_days` across all users.
pub async fn delete_stale(pool: &PgPool, max_age_days: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM insights
         WHERE created_at < now() - make_interval(days => $1)",
    )
    .bind(max_age_days)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
