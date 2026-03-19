// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct HealthRecordRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source: String,
    pub record_type: String,
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub source_id: Option<String>,
    pub source_instance: Option<String>,
    pub duplicate_of: Option<Uuid>,
    pub healthkit_written: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateHealthRecord {
    pub source: String,
    pub record_type: String,
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub source_id: Option<String>,
}

#[derive(Deserialize)]
pub struct HealthRecordQuery {
    pub record_type: Option<String>,
    pub source: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}
