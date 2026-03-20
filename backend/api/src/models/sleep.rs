// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct SleepRow {
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

#[derive(Deserialize)]
pub struct SleepQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}
