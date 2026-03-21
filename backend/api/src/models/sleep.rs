// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::observation::ObservationRow;

/// API response for a sleep record — same shape the web frontend expects.
#[derive(Serialize)]
pub struct SleepResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub sleep_start: Option<DateTime<Utc>>,
    pub sleep_end: Option<DateTime<Utc>>,
    pub duration_minutes: i32,
    pub deep_minutes: Option<i32>,
    pub light_minutes: Option<i32>,
    pub rem_minutes: Option<i32>,
    pub awake_minutes: Option<i32>,
    pub score: Option<i32>,
    pub source: String,
    pub source_id: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Request body for creating a sleep record.
#[derive(Deserialize)]
pub struct CreateSleep {
    pub date: NaiveDate,
    pub sleep_start: Option<DateTime<Utc>>,
    pub sleep_end: Option<DateTime<Utc>>,
    pub duration_minutes: i32,
    pub deep_minutes: Option<i32>,
    pub light_minutes: Option<i32>,
    pub rem_minutes: Option<i32>,
    pub awake_minutes: Option<i32>,
    pub score: Option<i32>,
    pub source: Option<String>,
    pub source_id: Option<String>,
    pub notes: Option<String>,
}

/// Query parameters for listing sleep records.
#[derive(Deserialize)]
pub struct SleepQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}

impl SleepResponse {
    /// Build a `SleepResponse` from an `ObservationRow` that has type = 'sleep'.
    pub fn from_observation(obs: ObservationRow) -> Self {
        let value = obs.value.unwrap_or_default();

        let date = obs.start_time.date_naive();
        let duration_minutes = value
            .get("duration_minutes")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let deep_minutes = value.get("deep_minutes").and_then(|v| v.as_i64()).map(|v| v as i32);
        let light_minutes = value.get("light_minutes").and_then(|v| v.as_i64()).map(|v| v as i32);
        let rem_minutes = value.get("rem_minutes").and_then(|v| v.as_i64()).map(|v| v as i32);
        let awake_minutes = value.get("awake_minutes").and_then(|v| v.as_i64()).map(|v| v as i32);
        let score = value.get("score").and_then(|v| v.as_i64()).map(|v| v as i32);
        let source_id = value
            .get("source_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let notes = value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        SleepResponse {
            id: obs.id,
            user_id: obs.user_id,
            date,
            sleep_start: Some(obs.start_time),
            sleep_end: obs.end_time,
            duration_minutes,
            deep_minutes,
            light_minutes,
            rem_minutes,
            awake_minutes,
            score,
            source: obs.source,
            source_id,
            notes,
            created_at: obs.created_at,
        }
    }
}
