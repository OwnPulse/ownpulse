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
    pub line_id: Uuid,
    pub day_number: i32,
}

#[derive(Deserialize)]
pub struct SkipDoseRequest {
    pub line_id: Uuid,
    pub day_number: i32,
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
    pub line_id: Uuid,
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
