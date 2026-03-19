// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::intervention::{CreateIntervention, InterventionRow};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Insert a new intervention.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    intervention: &CreateIntervention,
) -> Result<InterventionRow, sqlx::Error> {
    sqlx::query_as::<_, InterventionRow>(
        "INSERT INTO interventions
            (user_id, substance, dose, unit, route, administered_at,
             fasted, timing_relative_to, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id, user_id, substance, dose, unit, route,
                   administered_at, fasted, timing_relative_to, notes,
                   healthkit_written, created_at",
    )
    .bind(user_id)
    .bind(&intervention.substance)
    .bind(intervention.dose)
    .bind(&intervention.unit)
    .bind(&intervention.route)
    .bind(intervention.administered_at)
    .bind(intervention.fasted)
    .bind(&intervention.timing_relative_to)
    .bind(&intervention.notes)
    .fetch_one(pool)
    .await
}

/// List interventions for a user with optional time-range filters.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<Vec<InterventionRow>, sqlx::Error> {
    sqlx::query_as::<_, InterventionRow>(
        "SELECT id, user_id, substance, dose, unit, route,
                administered_at, fasted, timing_relative_to, notes,
                healthkit_written, created_at
         FROM interventions
         WHERE user_id = $1
           AND ($2::timestamptz IS NULL OR administered_at >= $2)
           AND ($3::timestamptz IS NULL OR administered_at <= $3)
         ORDER BY administered_at DESC
         LIMIT 1000",
    )
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Get a single intervention by id, scoped to user.
pub async fn get_by_id(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<InterventionRow, sqlx::Error> {
    sqlx::query_as::<_, InterventionRow>(
        "SELECT id, user_id, substance, dose, unit, route,
                administered_at, fasted, timing_relative_to, notes,
                healthkit_written, created_at
         FROM interventions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete an intervention. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM interventions WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
