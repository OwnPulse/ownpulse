// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::checkin::{CheckinRow, UpsertCheckin};
use sqlx::PgPool;
use uuid::Uuid;

/// Upsert a daily check-in. If one already exists for this user+date, update it.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    checkin: &UpsertCheckin,
) -> Result<CheckinRow, sqlx::Error> {
    sqlx::query_as::<_, CheckinRow>(
        "INSERT INTO daily_checkins
            (user_id, date, energy, mood, focus, recovery, libido, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (user_id, date) DO UPDATE SET
            energy   = EXCLUDED.energy,
            mood     = EXCLUDED.mood,
            focus    = EXCLUDED.focus,
            recovery = EXCLUDED.recovery,
            libido   = EXCLUDED.libido,
            notes    = EXCLUDED.notes
         RETURNING id, user_id, date, energy, mood, focus, recovery, libido,
                   notes, created_at",
    )
    .bind(user_id)
    .bind(checkin.date)
    .bind(checkin.energy)
    .bind(checkin.mood)
    .bind(checkin.focus)
    .bind(checkin.recovery)
    .bind(checkin.libido)
    .bind(&checkin.notes)
    .fetch_one(pool)
    .await
}

/// List check-ins for a user, newest first.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<CheckinRow>, sqlx::Error> {
    sqlx::query_as::<_, CheckinRow>(
        "SELECT id, user_id, date, energy, mood, focus, recovery, libido,
                notes, created_at
         FROM daily_checkins
         WHERE user_id = $1
         ORDER BY date DESC
         LIMIT 1000",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Get a single check-in by id, scoped to user.
pub async fn get_by_id(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<CheckinRow, sqlx::Error> {
    sqlx::query_as::<_, CheckinRow>(
        "SELECT id, user_id, date, energy, mood, focus, recovery, libido,
                notes, created_at
         FROM daily_checkins
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete a check-in. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM daily_checkins WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
