// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use sqlx::PgPool;
use uuid::Uuid;

use crate::models::explore::ChartRow;

/// Insert a new saved chart.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    config: &serde_json::Value,
) -> Result<ChartRow, sqlx::Error> {
    sqlx::query_as::<_, ChartRow>(
        "INSERT INTO explore_charts (user_id, name, config)
         VALUES ($1, $2, $3)
         RETURNING id, user_id, name, config, created_at, updated_at",
    )
    .bind(user_id)
    .bind(name)
    .bind(config)
    .fetch_one(pool)
    .await
}

/// List all saved charts for a user, newest first.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<ChartRow>, sqlx::Error> {
    sqlx::query_as::<_, ChartRow>(
        "SELECT id, user_id, name, config, created_at, updated_at
         FROM explore_charts
         WHERE user_id = $1
         ORDER BY updated_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Get a single chart by id, scoped to user (IDOR protection).
pub async fn get_by_id(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<ChartRow, sqlx::Error> {
    sqlx::query_as::<_, ChartRow>(
        "SELECT id, user_id, name, config, created_at, updated_at
         FROM explore_charts
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Update chart name and/or config. Returns the updated row.
pub async fn update(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    name: Option<&str>,
    config: Option<&serde_json::Value>,
) -> Result<ChartRow, sqlx::Error> {
    sqlx::query_as::<_, ChartRow>(
        "UPDATE explore_charts
         SET name = COALESCE($3, name),
             config = COALESCE($4, config),
             updated_at = now()
         WHERE id = $1 AND user_id = $2
         RETURNING id, user_id, name, config, created_at, updated_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(config)
    .fetch_one(pool)
    .await
}

/// Delete a chart. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM explore_charts WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
