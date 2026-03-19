// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct CheckinRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub energy: Option<i32>,
    pub mood: Option<i32>,
    pub focus: Option<i32>,
    pub recovery: Option<i32>,
    pub libido: Option<i32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpsertCheckin {
    pub date: NaiveDate,
    pub energy: Option<i32>,
    pub mood: Option<i32>,
    pub focus: Option<i32>,
    pub recovery: Option<i32>,
    pub libido: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct CheckinQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}
