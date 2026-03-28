// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreatePoll {
    pub name: String,
    pub custom_prompt: Option<String>,
    pub dimensions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePoll {
    pub name: Option<String>,
    pub custom_prompt: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PollRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub custom_prompt: Option<String>,
    pub dimensions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PollResponse {
    pub id: Uuid,
    pub name: String,
    pub custom_prompt: Option<String>,
    pub dimensions: Vec<String>,
    pub members: Vec<PollMemberView>,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PollMemberView {
    pub id: Uuid,
    pub observer_email: String,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InviteResponse {
    pub invite_token: Uuid,
    pub invite_expires_at: DateTime<Utc>,
    pub invite_url: String,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInvite {
    pub token: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct SubmitResponse {
    pub date: NaiveDate,
    pub scores: serde_json::Value,
}

#[derive(Debug, Serialize, FromRow)]
pub struct OwnerResponseView {
    pub id: Uuid,
    pub member_id: Uuid,
    pub observer_email: Option<String>,
    pub date: NaiveDate,
    pub scores: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ObserverResponseView {
    pub id: Uuid,
    pub date: NaiveDate,
    pub scores: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ObserverPollView {
    pub id: Uuid,
    pub owner_display: String,
    pub name: String,
    pub custom_prompt: Option<String>,
    pub dimensions: Vec<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ObserverPollRow {
    pub id: Uuid,
    pub owner_email: Option<String>,
    pub name: String,
    pub custom_prompt: Option<String>,
    pub dimensions: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MemberRow {
    pub id: Uuid,
    pub observer_email: Option<String>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ExportRow {
    pub id: Uuid,
    pub poll_name: String,
    pub date: NaiveDate,
    pub scores: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ResponseRow {
    pub id: Uuid,
    pub poll_id: Uuid,
    pub member_id: Uuid,
    pub date: NaiveDate,
    pub scores: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
