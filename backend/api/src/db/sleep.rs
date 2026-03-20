// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::sleep::{CreateSleep, SleepRow};

/// Insert a new sleep record.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    sleep: &CreateSleep,
) -> Result<SleepRow, sqlx::Error> {
    let source = sleep.source.as_deref().unwrap_or("manual");

    sqlx::query_as::<_, SleepRow>(
        "INSERT INTO sleep_records
            (user_id, date, sleep_start, sleep_end, duration_minutes,
             deep_minutes, light_minutes, rem_minutes, awake_minutes,
             score, source, source_id, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, user_id, date, sleep_start, sleep_end, duration_minutes,
                   deep_minutes, light_minutes, rem_minutes, awake_minutes,
                   score, source, source_id, notes, created_at",
    )
    .bind(user_id)
    .bind(sleep.date)
    .bind(sleep.sleep_start)
    .bind(sleep.sleep_end)
    .bind(sleep.duration_minutes)
    .bind(sleep.deep_minutes)
    .bind(sleep.light_minutes)
    .bind(sleep.rem_minutes)
    .bind(sleep.awake_minutes)
    .bind(sleep.score)
    .bind(source)
    .bind(&sleep.source_id)
    .bind(&sleep.notes)
    .fetch_one(pool)
    .await
}

/// List sleep records for a user with optional date-range filters, newest first.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<Vec<SleepRow>, sqlx::Error> {
    sqlx::query_as::<_, SleepRow>(
        "SELECT id, user_id, date, sleep_start, sleep_end, duration_minutes,
                deep_minutes, light_minutes, rem_minutes, awake_minutes,
                score, source, source_id, notes, created_at
         FROM sleep_records
         WHERE user_id = $1
           AND ($2::date IS NULL OR date >= $2)
           AND ($3::date IS NULL OR date <= $3)
         ORDER BY date DESC
         LIMIT 1000",
    )
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Get a single sleep record by id, scoped to user.
pub async fn get_by_id(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<SleepRow, sqlx::Error> {
    sqlx::query_as::<_, SleepRow>(
        "SELECT id, user_id, date, sleep_start, sleep_end, duration_minutes,
                deep_minutes, light_minutes, rem_minutes, awake_minutes,
                score, source, source_id, notes, created_at
         FROM sleep_records
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete a sleep record. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sleep_records WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
