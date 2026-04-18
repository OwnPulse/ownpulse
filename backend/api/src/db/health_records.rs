// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::health_record::{CreateHealthRecord, HealthRecordRow};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Find a potential duplicate: same user, same record_type, within 60 seconds
/// and 2% value tolerance, from a *different* source.
pub async fn find_duplicate(
    pool: &PgPool,
    user_id: Uuid,
    record: &CreateHealthRecord,
) -> Result<Option<HealthRecordRow>, sqlx::Error> {
    sqlx::query_as::<_, HealthRecordRow>(
        "SELECT id, user_id, source, record_type, value, unit,
                start_time, end_time, metadata, source_id, source_instance,
                duplicate_of, healthkit_written, created_at
         FROM health_records
         WHERE user_id = $1
           AND record_type = $2
           AND source <> $3
           AND start_time BETWEEN $4 - INTERVAL '60 seconds'
                               AND $4 + INTERVAL '60 seconds'
           AND ($5::double precision IS NULL
                OR value BETWEEN $5 * 0.98 AND $5 * 1.02)
         ORDER BY start_time DESC
         LIMIT 1",
    )
    .bind(user_id)
    .bind(&record.record_type)
    .bind(&record.source)
    .bind(record.start_time)
    .bind(record.value)
    .fetch_optional(pool)
    .await
}

/// Bulk insert HealthKit-sourced records in a single round trip via `UNNEST`.
///
/// Used by `POST /healthkit/sync` to avoid the per-record `find_duplicate` +
/// `insert` loop (200 DB round trips for a 100-record batch). Relies on the
/// `UNIQUE(user_id, source, record_type, start_time, source_id)` constraint on
/// `health_records` for same-source idempotency — duplicate batches replay
/// cleanly with zero new rows.
///
/// Cross-source deduplication (`duplicate_of`) is **not** computed here. That
/// logic is deferred to a future async reconciliation job; rows inserted via
/// this path always have `duplicate_of = NULL`. Source is forced to
/// `'healthkit'` in the SQL for defence in depth, independent of any value
/// passed in by the caller.
///
/// Returns the number of rows actually inserted (after `ON CONFLICT DO NOTHING`).
pub async fn bulk_insert_healthkit(
    pool: &PgPool,
    user_id: Uuid,
    records: &[CreateHealthRecord],
) -> Result<usize, sqlx::Error> {
    if records.is_empty() {
        return Ok(0);
    }

    // Build parallel column arrays for UNNEST. Each array has one entry per
    // record at the same index. Option<T> entries become NULLs in the array.
    let mut record_types: Vec<String> = Vec::with_capacity(records.len());
    let mut values: Vec<Option<f64>> = Vec::with_capacity(records.len());
    let mut units: Vec<Option<String>> = Vec::with_capacity(records.len());
    let mut start_times: Vec<DateTime<Utc>> = Vec::with_capacity(records.len());
    let mut end_times: Vec<Option<DateTime<Utc>>> = Vec::with_capacity(records.len());
    let mut metadatas: Vec<Option<serde_json::Value>> = Vec::with_capacity(records.len());
    let mut source_ids: Vec<Option<String>> = Vec::with_capacity(records.len());

    for r in records {
        record_types.push(r.record_type.clone());
        values.push(r.value);
        units.push(r.unit.clone());
        start_times.push(r.start_time);
        end_times.push(r.end_time);
        metadatas.push(r.metadata.clone());
        source_ids.push(r.source_id.clone());
    }

    // Single INSERT with UNNEST expanding the parallel arrays to rows.
    // source is forced to 'healthkit' in SQL — defence in depth.
    // ON CONFLICT DO NOTHING on the UNIQUE constraint makes this idempotent.
    // RETURNING 1 + fetch_all().len() gives us the count of newly inserted rows.
    let inserted = sqlx::query(
        "INSERT INTO health_records
            (user_id, source, record_type, value, unit, start_time, end_time,
             metadata, source_id)
         SELECT $1, 'healthkit', rt, v, u, st, et, md, sid
         FROM UNNEST(
             $2::text[],
             $3::double precision[],
             $4::text[],
             $5::timestamptz[],
             $6::timestamptz[],
             $7::jsonb[],
             $8::text[]
         ) AS t(rt, v, u, st, et, md, sid)
         ON CONFLICT (user_id, source, record_type, start_time, source_id)
             DO NOTHING
         RETURNING 1",
    )
    .bind(user_id)
    .bind(&record_types)
    .bind(&values)
    .bind(&units)
    .bind(&start_times)
    .bind(&end_times)
    .bind(&metadatas)
    .bind(&source_ids)
    .fetch_all(pool)
    .await?;

    Ok(inserted.len())
}

/// Insert a health record, optionally marking it as a duplicate of another.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    record: &CreateHealthRecord,
    duplicate_of: Option<Uuid>,
) -> Result<HealthRecordRow, sqlx::Error> {
    sqlx::query_as::<_, HealthRecordRow>(
        "INSERT INTO health_records
            (user_id, source, record_type, value, unit, start_time, end_time,
             metadata, source_id, source_instance, duplicate_of)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, user_id, source, record_type, value, unit,
                   start_time, end_time, metadata, source_id, source_instance,
                   duplicate_of, healthkit_written, created_at",
    )
    .bind(user_id)
    .bind(&record.source)
    .bind(&record.record_type)
    .bind(record.value)
    .bind(&record.unit)
    .bind(record.start_time)
    .bind(record.end_time)
    .bind(&record.metadata)
    .bind(&record.source_id)
    .bind(None::<String>) // source_instance — not on CreateHealthRecord, nullable in DB
    .bind(duplicate_of)
    .fetch_one(pool)
    .await
}

/// List health records for a user with optional filters. Capped at 1000 rows.
pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    record_type: Option<&str>,
    source: Option<&str>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<Vec<HealthRecordRow>, sqlx::Error> {
    sqlx::query_as::<_, HealthRecordRow>(
        "SELECT id, user_id, source, record_type, value, unit,
                start_time, end_time, metadata, source_id, source_instance,
                duplicate_of, healthkit_written, created_at
         FROM health_records
         WHERE user_id = $1
           AND ($2::text IS NULL OR record_type = $2)
           AND ($3::text IS NULL OR source = $3)
           AND ($4::timestamptz IS NULL OR start_time >= $4)
           AND ($5::timestamptz IS NULL OR start_time <= $5)
         ORDER BY start_time DESC
         LIMIT 1000",
    )
    .bind(user_id)
    .bind(record_type)
    .bind(source)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Get a single health record by id, scoped to user.
pub async fn get_by_id(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<HealthRecordRow, sqlx::Error> {
    sqlx::query_as::<_, HealthRecordRow>(
        "SELECT id, user_id, source, record_type, value, unit,
                start_time, end_time, metadata, source_id, source_instance,
                duplicate_of, healthkit_written, created_at
         FROM health_records
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Delete a health record. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    // Clear any duplicate_of references pointing to this record first.
    sqlx::query("UPDATE health_records SET duplicate_of = NULL WHERE duplicate_of = $1")
        .bind(id)
        .execute(pool)
        .await?;

    let result = sqlx::query("DELETE FROM health_records WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
