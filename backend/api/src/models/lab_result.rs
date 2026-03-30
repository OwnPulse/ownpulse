// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct LabResultRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub panel_date: NaiveDate,
    pub lab_name: Option<String>,
    pub marker: String,
    pub value: f64,
    pub unit: String,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub out_of_range: Option<bool>,
    pub source: String,
    pub source_id: Option<String>,
    pub uploaded_file_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateLabResult {
    pub panel_date: NaiveDate,
    pub lab_name: Option<String>,
    pub marker: String,
    pub value: f64,
    pub unit: String,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub source: Option<String>,
    pub source_id: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkCreateLabResults {
    pub records: Vec<CreateLabResult>,
}

#[derive(Deserialize)]
pub struct LabResultQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}
