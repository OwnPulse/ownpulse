// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::source_preference::SourcePreferenceRow;
use sqlx::PgPool;
use uuid::Uuid;

/// List all source preferences for a user.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<SourcePreferenceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourcePreferenceRow>(
        "SELECT id, user_id, metric_type, preferred_source, created_at
         FROM source_preferences
         WHERE user_id = $1
         ORDER BY metric_type",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Upsert a source preference. If one already exists for this user+metric_type,
/// update the preferred_source.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    metric_type: &str,
    preferred_source: &str,
) -> Result<SourcePreferenceRow, sqlx::Error> {
    sqlx::query_as::<_, SourcePreferenceRow>(
        "INSERT INTO source_preferences (user_id, metric_type, preferred_source)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id, metric_type) DO UPDATE SET
            preferred_source = EXCLUDED.preferred_source
         RETURNING id, user_id, metric_type, preferred_source, created_at",
    )
    .bind(user_id)
    .bind(metric_type)
    .bind(preferred_source)
    .fetch_one(pool)
    .await
}
