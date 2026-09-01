// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// --- Row types ---

#[derive(FromRow, Serialize)]
pub struct ProtocolRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub duration_days: i32,
    pub status: String,
    pub is_template: bool,
    pub tags: Option<serde_json::Value>,
    pub source_url: Option<String>,
    pub share_token: Option<String>,
    pub share_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize)]
pub struct ProtocolLineRow {
    pub id: Uuid,
    pub protocol_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub schedule_pattern: serde_json::Value,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize, Clone)]
pub struct ProtocolDoseRow {
    pub id: Uuid,
    pub protocol_line_id: Uuid,
    pub day_number: i32,
    pub status: String,
    pub intervention_id: Option<Uuid>,
    pub logged_at: DateTime<Utc>,
    /// The run this dose belongs to. `None` for legacy protocol-level doses
    /// logged before runs existed (or via the deprecated `/protocols/:id/doses/*`
    /// endpoints), so a protocol detail response can scope doses to a single run.
    pub run_id: Option<Uuid>,
    /// Optional free-text reason recorded when a dose is skipped.
    pub skip_reason: Option<String>,
}

#[derive(FromRow, Serialize, Clone)]
pub struct ProtocolRunRow {
    pub id: Uuid,
    pub protocol_id: Uuid,
    pub user_id: Uuid,
    pub start_date: NaiveDate,
    pub status: String,
    pub notify: bool,
    pub notify_time: Option<String>,
    pub notify_times: Option<serde_json::Value>,
    pub repeat_reminders: bool,
    pub repeat_interval_minutes: Option<i32>,
    pub created_at: DateTime<Utc>,
}

// --- Request types ---

#[derive(Deserialize)]
pub struct CreateProtocol {
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub duration_days: i32,
    pub lines: Vec<CreateProtocolLine>,
}

#[derive(Deserialize)]
pub struct CreateProtocolLine {
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub schedule_pattern: Vec<bool>,
    pub sort_order: i32,
}

#[derive(Deserialize)]
pub struct UpdateProtocol {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct LogDoseRequest {
    pub protocol_line_id: Uuid,
    pub day_number: i32,
    /// Optional explicit timestamp for the created intervention. Must fall
    /// within a day of the calendar date of `start_date + day_number`
    /// (evaluated in `tz_offset_minutes` if given). When omitted, a default
    /// time is derived from the line's `time_of_day`, in that same offset.
    pub administered_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    /// Caller's local UTC offset in minutes (e.g. `-420` for UTC-7), used to
    /// interpret "today"/the default dose time in the caller's own calendar
    /// day rather than UTC's. Range: -840..=840 (UTC-14:00..UTC+14:00).
    /// Defaults to UTC (`0`) when omitted.
    pub tz_offset_minutes: Option<i32>,
}

#[derive(Deserialize)]
pub struct SkipDoseRequest {
    pub protocol_line_id: Uuid,
    pub day_number: i32,
    pub skip_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateRunRequest {
    pub start_date: Option<NaiveDate>,
    pub notify: Option<bool>,
    pub notify_time: Option<String>,
    pub notify_times: Option<Vec<String>>,
    pub repeat_reminders: Option<bool>,
    pub repeat_interval_minutes: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateRunRequest {
    pub status: Option<String>,
    pub notify: Option<bool>,
    pub notify_time: Option<String>,
    pub notify_times: Option<Vec<String>>,
    pub repeat_reminders: Option<bool>,
    pub repeat_interval_minutes: Option<i32>,
}

// --- Response types ---

#[derive(Serialize)]
pub struct ProtocolResponse {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub duration_days: i32,
    pub status: String,
    pub is_template: bool,
    pub tags: Vec<String>,
    pub share_token: Option<String>,
    pub share_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub lines: Vec<ProtocolLineResponse>,
    pub runs: Vec<RunResponse>,
}

#[derive(Serialize)]
pub struct ProtocolLineResponse {
    pub id: Uuid,
    pub protocol_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub schedule_pattern: serde_json::Value,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub doses: Vec<ProtocolDoseRow>,
}

#[derive(Serialize)]
pub struct RunResponse {
    pub id: Uuid,
    pub protocol_id: Uuid,
    pub protocol_name: Option<String>,
    pub user_id: Uuid,
    pub start_date: NaiveDate,
    pub duration_days: Option<i32>,
    pub status: String,
    pub notify: bool,
    pub notify_time: Option<String>,
    pub notify_times: Option<serde_json::Value>,
    pub repeat_reminders: bool,
    pub repeat_interval_minutes: Option<i32>,
    pub progress_pct: f64,
    pub doses_today: i64,
    pub doses_completed_today: i64,
    /// `completed_closed / (scheduled_closed - skipped_closed) * 100`,
    /// rounded to 1 decimal place. "Closed" days are scheduled days
    /// strictly before today (`day_number < today_day`) that are not
    /// inside a pause interval — see `crate::dose_status`. `None` when the
    /// denominator is 0 (nothing scheduled yet, e.g. a run that starts in
    /// the future or was created today; or every closed day was skipped).
    /// Populated for `GET /protocols/runs/active` and run-creation
    /// responses; other run listings (e.g. `GET /protocols/:id/runs`) leave
    /// this `None` since they aren't in the hot "today" path.
    pub adherence_pct: Option<f64>,
    /// Count of closed scheduled days with no dose row (excluding paused
    /// days). `None` on the same placeholder paths as `adherence_pct`
    /// (kept `Option` rather than defaulting to `0` so a future path that
    /// genuinely can't compute it doesn't have to silently lie with `0`).
    pub doses_missed: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize)]
pub struct ProtocolListItem {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub start_date: Option<NaiveDate>,
    pub duration_days: i32,
    pub is_template: bool,
    pub tags: Option<serde_json::Value>,
    pub progress_pct: f64,
    pub next_dose: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow, Serialize)]
pub struct TodaysDoseItem {
    pub protocol_id: Uuid,
    pub protocol_name: String,
    pub run_id: Uuid,
    pub protocol_line_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub day_number: i32,
    pub status: Option<String>,
}

#[derive(FromRow, Serialize)]
pub struct ActiveSubstanceItem {
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub protocol_name: String,
}

#[derive(Serialize)]
pub struct ShareResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

// --- Adherence / dose-status types ---

/// Query params for `GET /protocols/runs/:run_id/doses`.
#[derive(Deserialize)]
pub struct DoseRangeQuery {
    pub from_day: Option<i32>,
    pub to_day: Option<i32>,
}

/// One entry of `GET /protocols/runs/:run_id/doses` — a single scheduled
/// (line, day) pair with its computed dose status.
#[derive(Serialize)]
pub struct RunDoseItem {
    pub day_number: i32,
    pub date: NaiveDate,
    pub protocol_line_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub status: String,
    pub dose_id: Option<Uuid>,
    pub intervention_id: Option<Uuid>,
    pub skip_reason: Option<String>,
    pub logged_at: Option<DateTime<Utc>>,
}

/// One entry of `GET /protocols/runs/missed-doses` — a scheduled day, in
/// the past, across the user's active runs, with no dose row.
#[derive(FromRow, Serialize)]
pub struct MissedDoseItem {
    pub protocol_id: Uuid,
    pub protocol_name: String,
    pub run_id: Uuid,
    pub protocol_line_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub day_number: i32,
    pub date: NaiveDate,
    pub status: String,
}

/// Row shape for the per-line adherence aggregate query.
#[derive(FromRow)]
pub struct LineAdherenceRow {
    pub protocol_line_id: Uuid,
    pub substance: String,
    pub scheduled_so_far: i64,
    pub completed: i64,
    pub skipped: i64,
    pub missed: i64,
}

/// Per-line breakdown in `GET /protocols/runs/:run_id/adherence`.
#[derive(Serialize)]
pub struct LineAdherence {
    pub protocol_line_id: Uuid,
    pub substance: String,
    pub scheduled_so_far: i64,
    pub completed: i64,
    pub skipped: i64,
    pub missed: i64,
    pub adherence_pct: Option<f64>,
}

/// Response body of `GET /protocols/runs/:run_id/adherence`.
#[derive(Serialize)]
pub struct AdherenceResponse {
    pub run_id: Uuid,
    pub scheduled_so_far: i64,
    pub completed: i64,
    pub skipped: i64,
    pub missed: i64,
    pub adherence_pct: Option<f64>,
    pub lines: Vec<LineAdherence>,
}

// --- Export/Import types ---

#[derive(Serialize, Deserialize)]
pub struct ProtocolExport {
    pub schema: String, // "ownpulse-protocol/v1"
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub duration_days: i32,
    pub lines: Vec<ProtocolLineExport>,
}

#[derive(Serialize, Deserialize)]
pub struct ProtocolLineExport {
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub time_of_day: Option<String>,
    pub pattern: serde_json::Value, // string shorthand or bool array
}

#[derive(Deserialize)]
pub struct PromoteRequest {
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct AdminBulkImportRequest {
    pub url: Option<String>,
    pub protocols: Option<Vec<ProtocolExport>>,
}

#[derive(Deserialize)]
pub struct CopyTemplateRequest {
    pub start_date: Option<NaiveDate>,
}

#[derive(FromRow, Serialize)]
pub struct TemplateListItem {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub duration_days: i32,
    pub tags: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// --- Notification preferences ---

#[derive(FromRow, Serialize)]
pub struct NotificationPreferencesRow {
    pub user_id: Uuid,
    pub default_notify: bool,
    pub default_notify_times: serde_json::Value,
    pub repeat_reminders: bool,
    pub repeat_interval_minutes: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdateNotificationPreferences {
    pub default_notify: Option<bool>,
    pub default_notify_times: Option<Vec<String>>,
    pub repeat_reminders: Option<bool>,
    pub repeat_interval_minutes: Option<i32>,
}

#[derive(Deserialize)]
pub struct RegisterPushTokenRequest {
    pub device_token: String,
    pub platform: String,
}

#[derive(FromRow, Serialize)]
pub struct PushTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_token: String,
    pub platform: String,
    pub created_at: DateTime<Utc>,
}
