// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct SourcePreferenceRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub metric_type: String,
    pub preferred_source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpsertSourcePreference {
    pub metric_type: String,
    pub preferred_source: String,
}
