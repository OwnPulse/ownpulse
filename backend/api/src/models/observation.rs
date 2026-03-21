// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

pub const VALID_OBSERVATION_TYPES: &[&str] = &[
    "event_instant",
    "event_duration",
    "scale",
    "symptom",
    "note",
    "context_tag",
    "environmental",
    "sleep",
];

pub fn is_valid_observation_type(t: &str) -> bool {
    VALID_OBSERVATION_TYPES.contains(&t)
}

#[derive(FromRow, Serialize)]
pub struct ObservationRow {
    pub id: Uuid,
    pub user_id: Uuid,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub obs_type: String,
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub value: Option<serde_json::Value>,
    pub source: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateObservation {
    #[serde(rename = "type")]
    pub obs_type: String,
    pub name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub value: Option<serde_json::Value>,
    pub source: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ObservationQuery {
    #[serde(rename = "type")]
    pub obs_type: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}
