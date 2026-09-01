// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// A single day's meeting aggregate — counts and minutes only, never event
/// titles/attendees/content. See `integrations::google_calendar` module docs
/// for the privacy boundary this reflects.
#[derive(FromRow, Serialize)]
pub struct CalendarDayRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub meeting_count: i32,
    pub meeting_minutes: i32,
    pub synced_at: DateTime<Utc>,
}
