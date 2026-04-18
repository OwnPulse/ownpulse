// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::health_record::{CreateHealthRecord, HealthRecordRow};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// A cross-source dedup match for a single record in a bulk-insert batch.
/// Emitted by [`bulk_insert_healthkit`] so the caller can log/metric each
/// match without re-querying the DB.
pub struct BulkDedupMatch {
    /// Index into the input `records` slice that this match refers to.
    pub batch_idx: usize,
    /// `record_type` copied from the input record (saves the caller a lookup).
    pub record_type: String,
    /// The id of the existing row we detected as a near-duplicate.
    pub existing_id: Uuid,
    /// The `source` of the existing row (e.g. `"garmin"`, `"oura"`).
    pub existing_source: String,
}

/// Result of a bulk HealthKit insert: number of rows actually written plus
/// the full set of cross-source duplicate matches detected for this batch.
pub struct BulkInsertResult {
    /// Rows inserted after `ON CONFLICT DO NOTHING` — same-source replays
    /// count zero here.
    pub inserted: usize,
    /// Cross-source near-duplicate matches (one per input record that matched
    /// an existing non-healthkit row within 60s / 2% tolerance). Each match
    /// has its `duplicate_of` column populated on the newly inserted row.
    pub duplicates: Vec<BulkDedupMatch>,
}

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

/// Bulk insert HealthKit-sourced records with batched cross-source dedup.
///
/// Used by `POST /healthkit/sync` to avoid the per-record `find_duplicate` +
/// `insert` loop that used to cost 200 DB round trips per 100-record batch.
/// This function does exactly **two** round trips per call:
///
/// 1. A preflight `UNNEST`-driven `SELECT` that returns, for each input row,
///    the closest existing record from a *different* source within 60 seconds
///    and 2% value tolerance (the project's deduplication rule — see
///    `CLAUDE.md`).
/// 2. A single `INSERT ... SELECT FROM UNNEST(...)` that writes the whole
///    batch. Each new row's `duplicate_of` is populated from the preflight
///    result at the same batch index. `ON CONFLICT DO NOTHING` on
///    `UNIQUE(user_id, source, record_type, start_time, source_id)` makes
///    same-source replays a no-op.
///
/// Source is forced to `'healthkit'` in the SQL for defence in depth,
/// independent of any value passed by the caller. The caller is responsible
/// for logging each [`BulkDedupMatch`] in the returned result — this layer
/// does not log because it has no handle to tracing conventions.
pub async fn bulk_insert_healthkit(
    pool: &PgPool,
    user_id: Uuid,
    records: &[CreateHealthRecord],
) -> Result<BulkInsertResult, sqlx::Error> {
    if records.is_empty() {
        return Ok(BulkInsertResult {
            inserted: 0,
            duplicates: Vec::new(),
        });
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

    // --- Step 1: preflight cross-source dedup.
    //
    // For each row in the input batch, find the closest existing record from
    // a *different* source within the 60-second / 2% tolerance window. We use
    // `WITH ORDINALITY` on the UNNEST so the lateral subquery can project a
    // stable 1-based index back into the input batch; that beats keying on
    // `(record_type, start_time)` because duplicate keys can appear within a
    // single batch (same device logging the same metric at the same instant).
    //
    // PostgreSQL enforces a uniform length across all arrays passed to one
    // UNNEST call. All column arrays here are built from the same `records`
    // slice, so their lengths are equal by construction.
    let dedup_rows: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT hr.id AS existing_id,
                hr.source AS existing_source,
                b.idx AS batch_idx
         FROM UNNEST($2::text[], $3::timestamptz[], $4::double precision[])
              WITH ORDINALITY AS b(record_type, start_time, value, idx)
         CROSS JOIN LATERAL (
             SELECT id, source
             FROM health_records
             WHERE user_id = $1
               AND record_type = b.record_type
               AND source <> 'healthkit'
               AND start_time BETWEEN b.start_time - INTERVAL '60 seconds'
                                  AND b.start_time + INTERVAL '60 seconds'
               AND (b.value IS NULL
                    OR value BETWEEN b.value * 0.98 AND b.value * 1.02)
             ORDER BY ABS(EXTRACT(EPOCH FROM (start_time - b.start_time)))
             LIMIT 1
         ) hr",
    )
    .bind(user_id)
    .bind(&record_types)
    .bind(&start_times)
    .bind(&values)
    .fetch_all(pool)
    .await?;

    // Map batch_idx (1-based from WITH ORDINALITY, converted to 0-based) to
    // the existing row's (id, source). `HashMap` is fine here — at most N
    // entries, all in-memory, no DB work.
    let mut dedup_by_idx: HashMap<usize, (Uuid, String)> = HashMap::new();
    let mut duplicates: Vec<BulkDedupMatch> = Vec::with_capacity(dedup_rows.len());
    for (existing_id, existing_source, idx_1based) in dedup_rows {
        // UNNEST...WITH ORDINALITY is always >= 1, so this subtraction is safe.
        let batch_idx = (idx_1based as usize).saturating_sub(1);
        if batch_idx >= records.len() {
            // Defensive: shouldn't happen (ordinality is bounded by array
            // length) but we never want to panic on adversarial DB input.
            continue;
        }
        dedup_by_idx.insert(batch_idx, (existing_id, existing_source.clone()));
        duplicates.push(BulkDedupMatch {
            batch_idx,
            record_type: records[batch_idx].record_type.clone(),
            existing_id,
            existing_source,
        });
    }

    // Build the parallel `duplicate_of` array aligned with the other input
    // arrays. `None` -> NULL in Postgres.
    let mut duplicate_ofs: Vec<Option<Uuid>> = Vec::with_capacity(records.len());
    for i in 0..records.len() {
        duplicate_ofs.push(dedup_by_idx.get(&i).map(|(id, _)| *id));
    }

    // --- Step 2: single INSERT with UNNEST expanding the parallel arrays to rows.
    //
    // `source` is hard-coded as the SQL literal `'healthkit'` in the SELECT
    // projection — we never bind user-controlled input for that column. The
    // route handler also forces `record.source = "healthkit"` on the way in
    // as belt-and-braces defence in depth (see `routes/healthkit.rs`), but
    // even if that check were somehow bypassed the DB query physically
    // cannot write a different source for this code path.
    //
    // `ON CONFLICT DO NOTHING` on the UNIQUE constraint makes same-source
    // replays a no-op. We only need the *count* of newly inserted rows
    // (never the IDs — `duplicate_of` is computed in Step 1 and fed as an
    // input column here), so `execute().rows_affected()` is the correct
    // primitive: no extra data on the wire compared to `RETURNING 1`.
    let rows_affected = sqlx::query(
        "INSERT INTO health_records
            (user_id, source, record_type, value, unit, start_time, end_time,
             metadata, source_id, duplicate_of)
         SELECT $1, 'healthkit', rt, v, u, st, et, md, sid, dof
         FROM UNNEST(
             $2::text[],
             $3::double precision[],
             $4::text[],
             $5::timestamptz[],
             $6::timestamptz[],
             $7::jsonb[],
             $8::text[],
             $9::uuid[]
         ) AS t(rt, v, u, st, et, md, sid, dof)
         ON CONFLICT (user_id, source, record_type, start_time, source_id)
             DO NOTHING",
    )
    .bind(user_id)
    .bind(&record_types)
    .bind(&values)
    .bind(&units)
    .bind(&start_times)
    .bind(&end_times)
    .bind(&metadatas)
    .bind(&source_ids)
    .bind(&duplicate_ofs)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(BulkInsertResult {
        inserted: rows_affected as usize,
        duplicates,
    })
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
