// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use metrics::{counter, histogram};

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::health_records as db_hr;
use crate::db::healthkit as db;
use crate::error::ApiError;
use crate::models::healthkit::{
    HealthKitBulkAck, HealthKitBulkInsert, HealthKitConfirm, HealthKitWriteQueueRow,
};

/// Maximum number of records accepted in a single `POST /healthkit/sync` call.
///
/// iOS currently chunks by 100 records per batch, so a cap of 500 leaves
/// headroom for future client changes while preventing a pathological client
/// from starving the pool or blowing up process memory on the two large
/// `UNNEST` bindings we build in [`db_hr::bulk_insert_healthkit`]. Raise this
/// only alongside a load test at the new ceiling.
const MAX_HEALTHKIT_BATCH: usize = 500;

/// `POST /healthkit/sync` — bulk insert health records from HealthKit in two
/// round trips.
///
/// Under the hood this is a two-query set-based path inside
/// [`db_hr::bulk_insert_healthkit`]:
///
/// 1. **Preflight**: one `UNNEST`-driven `SELECT` that returns any existing
///    cross-source near-duplicates for rows in the batch (same user, same
///    `record_type`, within 60 seconds, within 2% value tolerance, from a
///    different `source`). This implements the project-wide deduplication
///    rule defined in `CLAUDE.md`.
/// 2. **Insert**: one `INSERT ... SELECT FROM UNNEST(...)` that writes the
///    whole batch. Each new row's `duplicate_of` is populated from the
///    preflight result; `ON CONFLICT DO NOTHING` on
///    `UNIQUE(user_id, source, record_type, start_time, source_id)` makes
///    same-source replays a no-op.
///
/// `source` is hard-coded to `'healthkit'` in the SQL itself; we also mutate
/// every incoming record's `source` field to `"healthkit"` here before the DB
/// call as defence in depth. Either layer alone is sufficient to uphold the
/// invariant; both together mean attacker input never reaches the `source`
/// column.
///
/// For every detected cross-source match we emit a structured warning and
/// increment `healthkit_duplicates_detected_total` labelled by `record_type`.
/// For rows silently dropped by `ON CONFLICT DO NOTHING` (same-source replays
/// on `source_id`) we log at info level and increment
/// `healthkit_same_source_duplicates_total`.
///
/// Responds with `201 Created` and a small JSON ack
/// ([`HealthKitBulkAck`]). iOS currently ignores the body via
/// `requestNoContent`.
pub async fn bulk_insert(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(mut body): Json<HealthKitBulkInsert>,
) -> Result<(StatusCode, Json<HealthKitBulkAck>), ApiError> {
    let received = body.records.len();
    histogram!("healthkit_sync_batch_size").record(received as f64);

    // Reject oversized batches before we allocate any per-record Vec<_> in
    // the DB layer. iOS currently chunks at 100; 500 is a generous ceiling.
    if received > MAX_HEALTHKIT_BATCH {
        return Err(ApiError::BadRequest(format!(
            "batch size {received} exceeds maximum of {MAX_HEALTHKIT_BATCH}"
        )));
    }

    // Defence in depth: clobber any client-supplied `source` before it reaches
    // the DB layer. The SQL in `bulk_insert_healthkit` also hard-codes
    // `'healthkit'` as a literal in the INSERT projection, so a bypass of
    // this loop still cannot write a different source. We do it here anyway
    // because the preflight SELECT uses `source <> 'healthkit'` and future
    // callers might plausibly key behaviour off `record.source`.
    for record in &mut body.records {
        record.source = "healthkit".to_string();
    }

    let result = db_hr::bulk_insert_healthkit(&state.pool, user_id, &body.records).await?;

    // Log + meter every cross-source duplicate match. Shape matches the
    // pre-bulk per-record warning: user_id, existing row id/source,
    // new source ("healthkit"), and record_type. No health *values* in logs.
    for dup in &result.duplicates {
        tracing::warn!(
            user_id = %user_id,
            existing_id = %dup.existing_id,
            existing_source = %dup.existing_source,
            new_source = "healthkit",
            record_type = %dup.record_type,
            "duplicate health record detected during healthkit sync"
        );
        counter!(
            "healthkit_duplicates_detected_total",
            "record_type" => dup.record_type.clone()
        )
        .increment(1);
    }

    // Preserve the per-type breakdown on `healthkit_records_ingested_total`
    // that the pre-bulk code emitted. Grafana dashboards and alerts key on
    // the `record_type` label; emitting one counter per batch labelled only
    // by `source` silently broke them. Group the *input* batch by type and
    // emit one increment per type.
    //
    // Note: we credit the input distribution, not the actually-inserted rows.
    // `ON CONFLICT DO NOTHING` may drop some rows, but we have no cheap way
    // to learn which types they were. The discrepancy is captured by
    // `healthkit_same_source_duplicates_total` below. For dashboards that
    // need exact inserted-per-type counts, see the deferred reconciliation
    // job; until then, this matches pre-PR behaviour where every ingested
    // record bumped the per-type counter regardless of subsequent conflict.
    let mut by_type: HashMap<&str, u64> = HashMap::new();
    for r in &body.records {
        *by_type.entry(r.record_type.as_str()).or_insert(0) += 1;
    }
    for (record_type, n) in by_type {
        counter!(
            "healthkit_records_ingested_total",
            "record_type" => record_type.to_string(),
            "source" => "healthkit"
        )
        .increment(n);
    }

    // Same-source drops: rows that collided on
    // `UNIQUE(user_id, source, record_type, start_time, source_id)` and were
    // silently discarded by `ON CONFLICT DO NOTHING`. Expected on idempotent
    // iOS retries, but the old code path logged a warning through
    // `find_duplicate`; we preserve a tally so it's still visible.
    //
    // received = total input rows
    // result.inserted = rows the DB actually wrote
    // result.duplicates = cross-source matches (all are in inserted — they
    //   got `duplicate_of` set but still landed)
    // Same-source drops = received - inserted (saturating to avoid wraparound
    //   on the pathological case of result.inserted > received, which
    //   shouldn't happen but we defend against).
    let same_source_dropped = received.saturating_sub(result.inserted);
    if same_source_dropped > 0 {
        tracing::info!(
            received,
            inserted = result.inserted,
            same_source_dropped,
            "healthkit bulk insert dropped same-source duplicates"
        );
        counter!("healthkit_same_source_duplicates_total").increment(same_source_dropped as u64);
    }

    let ack = HealthKitBulkAck {
        received,
        inserted: result.inserted,
        duplicates: result.duplicates.len(),
    };
    Ok((StatusCode::CREATED, Json(ack)))
}

/// GET /healthkit/write-queue — pending items for the iOS client to write to HealthKit.
pub async fn write_queue(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<HealthKitWriteQueueRow>>, ApiError> {
    let rows = db::get_pending(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /healthkit/confirm — confirm that items have been written to HealthKit.
pub async fn confirm(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<HealthKitConfirm>,
) -> Result<StatusCode, ApiError> {
    db::confirm(&state.pool, user_id, &body.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}
