// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::observation::{CreateObservation, ObservationRow};
use sqlx::PgPool;
use uuid::Uuid;

/// Insert a new observation.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    obs: &CreateObservation,
) -> Result<ObservationRow, sqlx::Error> {
    sqlx::query_as::<_, ObservationRow>(
        r#"INSERT INTO observations
            (user_id, type, name, start_time, end_time, value, source, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, user_id, type as "obs_type", name, start_time, end_time,
                   value, source, metadata, created_at"#,
    )
    .bind(user_id)
    .bind(&obs.obs_type)
    .bind(&obs.name)
    .bind(obs.start_time)
    .bind(obs.end_time)
    .bind(&obs.value)
    .bind(&obs.source)
    .bind(&obs.metadata)
    .fetch_one(pool)
    .await
}

/// List observations for a user, optionally filtered by type.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    obs_type: Option<&str>,
) -> Result<Vec<ObservationRow>, sqlx::Error> {
    sqlx::query_as::<_, ObservationRow>(
        r#"SELECT id, user_id, type as "obs_type", name, start_time, end_time,
                  value, source, metadata, created_at
           FROM observations
           WHERE user_id = $1
             AND ($2::text IS NULL OR type = $2)
           ORDER BY start_time DESC
           LIMIT 1000"#,
    )
    .bind(user_id)
    .bind(obs_type)
    .fetch_all(pool)
    .await
}

/// Get a single observation by id, scoped to user.
pub async fn get_by_id(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<ObservationRow, sqlx::Error> {
    sqlx::query_as::<_, ObservationRow>(
        r#"SELECT id, user_id, type as "obs_type", name, start_time, end_time,
                  value, source, metadata, created_at
           FROM observations
           WHERE id = $1 AND user_id = $2"#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete an observation. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM observations WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
