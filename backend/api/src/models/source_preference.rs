// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct SourcePreferenceRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub metric_type: String,
    pub preferred_source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpsertSourcePreference {
    pub metric_type: String,
    pub preferred_source: String,
}

/// The full set of `source` values that can appear on a `health_records` row:
/// every integration sync job's hardcoded source literal, plus `"manual"`
/// (freeform user entry) and `"healthkit"` (the cycle-guarded write-back
/// source, see ADR-0008). A `preferred_source` outside this list can never
/// match any real row, so `POST /source-preferences` rejects it — a typo'd
/// preference should fail loudly rather than silently becoming inert in the
/// dedup-collapse logic (`SOURCE_PREFERENCE_EXCLUSION`).
pub const KNOWN_HEALTH_RECORD_SOURCES: &[&str] = &["garmin", "oura", "manual", "healthkit"];

/// One competing source for a metric in the overlap-scan result.
#[derive(Serialize)]
pub struct OverlapSource {
    pub source: String,
    pub record_count: i64,
}

/// A single metric that has overlapping records from more than one source
/// over the scan window.
#[derive(Serialize)]
pub struct OverlapMetric {
    pub metric_type: String,
    /// All sources contributing records for this metric, ordered by descending
    /// record count.
    pub sources: Vec<OverlapSource>,
}

/// Response body for `GET /sources/overlap-scan`.
#[derive(Serialize)]
pub struct OverlapScanResponse {
    pub metrics: Vec<OverlapMetric>,
}
