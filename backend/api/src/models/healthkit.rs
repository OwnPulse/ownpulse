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
