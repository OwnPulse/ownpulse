// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::intervention::{CreateIntervention, InterventionRow, UpdateIntervention};
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
                   healthkit_written, created_at, updated_at",
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
                healthkit_written, created_at, updated_at
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
                healthkit_written, created_at, updated_at
         FROM interventions
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Update the mutable fields of an intervention (COALESCE — unset fields are
/// left unchanged). Returns `None` if no row matched (not found, or not
/// owned by this user).
pub async fn update(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
    req: &UpdateIntervention,
) -> Result<Option<InterventionRow>, sqlx::Error> {
    sqlx::query_as::<_, InterventionRow>(
        "UPDATE interventions
         SET substance = COALESCE($3, substance),
             dose = COALESCE($4, dose),
             unit = COALESCE($5, unit),
             route = COALESCE($6, route),
             administered_at = COALESCE($7, administered_at),
             fasted = COALESCE($8, fasted),
             timing_relative_to = COALESCE($9, timing_relative_to),
             notes = COALESCE($10, notes),
             updated_at = now()
         WHERE id = $1 AND user_id = $2
         RETURNING id, user_id, substance, dose, unit, route,
                   administered_at, fasted, timing_relative_to, notes,
                   healthkit_written, created_at, updated_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(&req.substance)
    .bind(req.dose)
    .bind(&req.unit)
    .bind(&req.route)
    .bind(req.administered_at)
    .bind(req.fasted)
    .bind(&req.timing_relative_to)
    .bind(&req.notes)
    .fetch_optional(pool)
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
