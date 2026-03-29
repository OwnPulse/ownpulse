// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct InsightRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub insight_type: String,
    pub headline: String,
    pub detail: Option<String>,
    pub metadata: serde_json::Value,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
