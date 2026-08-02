// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::healthkit::{HealthKitWriteQueueRow, WriteFailure};
use sqlx::PgPool;
use uuid::Uuid;

/// `error` strings originate from the client (whatever HealthKit/iOS reports
/// for the failure) — cap the length so a misbehaving client can't stuff
/// unbounded text into the DB via this column.
const MAX_ERROR_LEN: usize = 500;

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
///
/// Takes a transaction (not a bare pool) so callers can run this alongside
/// `mark_failed` atomically — see the doc on that function for why a single
/// confirm+failures request must not partially apply.
///
/// Guards on both `confirmed_at IS NULL` and `failed_at IS NULL`: a row already
/// marked failed must not be silently re-confirmed by a later, unrelated
/// request that happens to still list its id in `ids` (e.g. a stale client
/// retry queued before the failure was reported). `mark_failed` has the
/// mirror-image guard (`confirmed_at IS NULL`), so within a single request a
/// row present in both `ids` and `failures` resolves to confirmed — `confirm`
/// runs first and clears `confirmed_at`, so `mark_failed`'s guard then skips it.
pub async fn confirm(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE healthkit_write_queue
         SET confirmed_at = now()
         WHERE user_id = $1 AND id = ANY($2)
           AND confirmed_at IS NULL
           AND failed_at IS NULL",
    )
    .bind(user_id)
    .bind(ids)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

/// Mark entries as failed (client attempted but could not write to HealthKit).
///
/// User-scoped like `confirm` — a caller can only fail their own queue rows.
/// This also unblocks `get_pending`'s `LIMIT 100`: without a way to mark
/// unwritable items failed, a handful of permanently-broken entries (e.g. a
/// HealthKit type no longer authorized) would sit at the head of the queue
/// forever, since `get_pending` excludes `failed_at IS NOT NULL` but never
/// excludes items simply confirmed-never. Returns the number of rows updated.
///
/// Takes the same transaction `confirm` ran in, so a single `POST /confirm`
/// request applies both `ids` and `failures` atomically — a crash between the
/// two UPDATEs must not leave the request half-applied.
///
/// Deduplicates `failures` by id before binding the `UNNEST` arrays: if a
/// client sends the same id twice with different error text, `UPDATE ...
/// FROM UNNEST(...)` would otherwise apply both source rows to the same
/// target row in a nondeterministic order (Postgres does not guarantee which
/// `FROM` match wins), storing a nondeterministic error string. Keeping the
/// *last* occurrence matches "most recent report wins" without depending on
/// join order.
pub async fn mark_failed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    failures: &[WriteFailure],
) -> Result<u64, sqlx::Error> {
    if failures.is_empty() {
        return Ok(0);
    }

    let mut by_id: std::collections::HashMap<Uuid, &str> = std::collections::HashMap::new();
    for f in failures {
        by_id.insert(f.id, f.error.as_str());
    }

    let ids: Vec<Uuid> = by_id.keys().copied().collect();
    let errors: Vec<String> = ids
        .iter()
        .map(|id| truncate_chars(by_id[id], MAX_ERROR_LEN))
        .collect();

    let result = sqlx::query(
        "UPDATE healthkit_write_queue AS q
         SET failed_at = now(), error = f.error
         FROM UNNEST($2::uuid[], $3::text[]) AS f(id, error)
         WHERE q.id = f.id AND q.user_id = $1
           AND q.confirmed_at IS NULL",
    )
    .bind(user_id)
    .bind(&ids)
    .bind(&errors)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

/// Truncate on char boundaries (not `String::truncate`, which is byte-based
/// and panics on a multi-byte UTF-8 boundary) — client-supplied error text
/// may contain non-ASCII.
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Enqueue a new HealthKit write-back entry.
///
/// **Cycle guard (ADR-0008, unconditional):** records whose originating
/// `source` is `"healthkit"` are *never* enqueued for write-back. Writing a
/// HealthKit-sourced record back to HealthKit would create a read→write→read
/// cycle that duplicates data indefinitely. This guard lives here — the single
/// chokepoint through which every write-queue insertion flows — rather than in
/// any route handler, so it cannot be bypassed by a new caller or by any API
/// parameter. It is not configurable. Callers pass the record's `source`; when
/// it is `"healthkit"` this function returns `Ok(())` without inserting a row.
pub async fn enqueue_write(
    pool: &PgPool,
    user_id: Uuid,
    source: &str,
    hk_type: &str,
    value: &serde_json::Value,
    source_record_id: Option<Uuid>,
    source_table: Option<&str>,
) -> Result<(), sqlx::Error> {
    // Unconditional cycle guard: never write HealthKit-sourced records back to
    // HealthKit. See ADR-0008. No-op (not an error) so callers stay simple.
    if source == "healthkit" {
        return Ok(());
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_chars_leaves_short_strings_untouched() {
        assert_eq!(truncate_chars("short", 500), "short");
    }

    #[test]
    fn truncate_chars_truncates_ascii_by_char_count() {
        let input = "x".repeat(1000);
        let result = truncate_chars(&input, 500);
        assert_eq!(result.chars().count(), 500);
        assert_eq!(result.len(), 500); // 1 byte/char for ASCII
    }

    #[test]
    fn truncate_chars_truncates_multibyte_by_char_count_not_byte_count() {
        // 'é' is 2 bytes in UTF-8. Truncating by byte count would either
        // panic (String::truncate on a non-char-boundary) or silently cut a
        // char in half; truncating by char count must yield exactly 500
        // chars (1000 bytes), not 500 bytes (250 chars).
        let input = "é".repeat(1000);
        let result = truncate_chars(&input, 500);
        assert_eq!(result.chars().count(), 500);
        assert_eq!(result.len(), 1000);
    }

    #[test]
    fn truncate_chars_max_zero_yields_empty_string() {
        assert_eq!(truncate_chars("anything", 0), "");
    }
}
