// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

/// Upsert the meeting aggregate for a single user-day.
///
/// `calendar_days` rows are recomputed wholesale from the source calendar
/// events on every sync, not accumulated — so this always *overwrites* the
/// existing row for `(user_id, date)` rather than adding to it. A meeting
/// that was cancelled or rescheduled upstream is reflected correctly on the
/// next sync instead of leaving a stale, too-high count forever. Relies on
/// the `UNIQUE(user_id, date)` constraint on `calendar_days` (0001_init.sql).
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    date: NaiveDate,
    meeting_count: i32,
    meeting_minutes: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO calendar_days (user_id, date, meeting_count, meeting_minutes, synced_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (user_id, date) DO UPDATE SET
            meeting_count = EXCLUDED.meeting_count,
            meeting_minutes = EXCLUDED.meeting_minutes,
            synced_at = now()",
    )
    .bind(user_id)
    .bind(date)
    .bind(meeting_count)
    .bind(meeting_minutes)
    .execute(pool)
    .await?;
    Ok(())
}
