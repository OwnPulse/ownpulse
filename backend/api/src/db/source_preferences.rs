// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::source_preference::{OverlapMetric, OverlapSource, SourcePreferenceRow};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Number of days of history the overlap scan considers.
const OVERLAP_SCAN_WINDOW_DAYS: i32 = 30;

/// Scan the last [`OVERLAP_SCAN_WINDOW_DAYS`] days of the user's health
/// records and return, per metric (`record_type`), every source contributing
/// records — but only for metrics that have records from more than one source.
///
/// Sources within a metric are ordered by descending record count. Only the
/// owning user's data is scanned (filtered by `user_id`), so this never
/// crosses the cooperative data boundary. Records marked as duplicates
/// (`duplicate_of IS NOT NULL`) are intentionally *included*: a record being
/// deduped against another source is exactly the signal that two sources are
/// competing for this metric, which is what the wizard exists to resolve.
pub async fn overlap_scan(pool: &PgPool, user_id: Uuid) -> Result<Vec<OverlapMetric>, sqlx::Error> {
    // Aggregate per (record_type, source), ordered so rows for the same metric
    // are contiguous and the highest-count source comes first. The
    // "more than one source" filter is applied in Rust after grouping.
    let rows = sqlx::query(
        "SELECT record_type, source, COUNT(*) AS record_count
         FROM health_records
         WHERE user_id = $1
           AND start_time >= now() - make_interval(days => $2)
         GROUP BY record_type, source
         ORDER BY record_type, COUNT(*) DESC, source",
    )
    .bind(user_id)
    .bind(OVERLAP_SCAN_WINDOW_DAYS)
    .fetch_all(pool)
    .await?;

    // Group consecutive rows by record_type (the query is ordered by
    // record_type), keeping only metrics with >1 distinct source.
    let mut metrics: Vec<OverlapMetric> = Vec::new();
    for row in rows {
        let record_type: String = row.get("record_type");
        let source: String = row.get("source");
        let record_count: i64 = row.get("record_count");

        match metrics.last_mut() {
            Some(metric) if metric.metric_type == record_type => {
                metric.sources.push(OverlapSource {
                    source,
                    record_count,
                });
            }
            _ => {
                metrics.push(OverlapMetric {
                    metric_type: record_type,
                    sources: vec![OverlapSource {
                        source,
                        record_count,
                    }],
                });
            }
        }
    }

    metrics.retain(|m| m.sources.len() > 1);
    Ok(metrics)
}

/// List all source preferences for a user.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<SourcePreferenceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourcePreferenceRow>(
        "SELECT id, user_id, metric_type, preferred_source, created_at
         FROM source_preferences
         WHERE user_id = $1
         ORDER BY metric_type",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Upsert a source preference. If one already exists for this user+metric_type,
/// update the preferred_source.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    metric_type: &str,
    preferred_source: &str,
) -> Result<SourcePreferenceRow, sqlx::Error> {
    sqlx::query_as::<_, SourcePreferenceRow>(
        "INSERT INTO source_preferences (user_id, metric_type, preferred_source)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id, metric_type) DO UPDATE SET
            preferred_source = EXCLUDED.preferred_source
         RETURNING id, user_id, metric_type, preferred_source, created_at",
    )
    .bind(user_id)
    .bind(metric_type)
    .bind(preferred_source)
    .fetch_one(pool)
    .await
}
