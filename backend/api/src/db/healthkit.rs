// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::healthkit::HealthKitWriteQueueRow;
use sqlx::PgPool;
use uuid::Uuid;

/// Get pending HealthKit write-back entries (not yet confirmed or failed).
pub async fn get_pending(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<HealthKitWriteQueueRow>, sqlx::Error> {
    sqlx::query_as::<_, HealthKitWriteQueueRow>(
        "SELECT id, user_id, hk_type, value, scheduled_at,
                confirmed_at, failed_at, error, source_record_id, source_table
         FROM healthkit_write_queue
         WHERE user_id = $1
           AND confirmed_at IS NULL
           AND failed_at IS NULL
         ORDER BY scheduled_at ASC
         LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Mark entries as confirmed (written to HealthKit). Returns the number of rows updated.
pub async fn confirm(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE healthkit_write_queue
         SET confirmed_at = now()
         WHERE user_id = $1 AND id = ANY($2)
           AND confirmed_at IS NULL",
    )
    .bind(user_id)
    .bind(ids)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Enqueue a new HealthKit write-back entry.
pub async fn enqueue_write(
    pool: &PgPool,
    user_id: Uuid,
    hk_type: &str,
    value: &serde_json::Value,
    source_record_id: Option<Uuid>,
    source_table: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO healthkit_write_queue
            (user_id, hk_type, value, source_record_id, source_table)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(hk_type)
    .bind(value)
    .bind(source_record_id)
    .bind(source_table)
    .execute(pool)
    .await?;
    Ok(())
}
