// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct HealthKitWriteQueueRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub hk_type: String,
    pub value: serde_json::Value,
    pub scheduled_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub source_record_id: Option<Uuid>,
    pub source_table: Option<String>,
}

#[derive(Deserialize)]
pub struct HealthKitBulkInsert {
    pub records: Vec<crate::models::health_record::CreateHealthRecord>,
}

#[derive(Deserialize)]
pub struct HealthKitConfirm {
    pub ids: Vec<Uuid>,
}

/// Acknowledgement returned by `POST /api/v1/healthkit/sync`.
///
/// Exists so the response body is an honest typed contract (the previous
/// version lied, declaring `Vec<HealthRecordRow>` and always returning
/// `[]`). iOS currently ignores the body via `requestNoContent` but may
/// later surface `duplicates` in a sync-status UI without another wire
/// change.
///
/// - `received` is the number of records the server *accepted* from the
///   request body — always equal to `body.records.len()` for accepted
///   requests (oversized batches are rejected before this struct is built).
/// - `inserted` is the number of rows actually written (post
///   `ON CONFLICT DO NOTHING`). Same-source replays do not count here.
/// - `duplicates` is the number of cross-source near-duplicates detected
///   and marked via `duplicate_of`. These *are* included in `inserted` —
///   they land in the DB with a `duplicate_of` reference to the existing
///   non-healthkit row — they're just flagged.
#[derive(Serialize)]
pub struct HealthKitBulkAck {
    pub received: usize,
    pub inserted: usize,
    pub duplicates: usize,
}
