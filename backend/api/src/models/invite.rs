// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
pub struct InviteRow {
    pub id: Uuid,
    pub code: String,
    pub created_by: Uuid,
    pub label: Option<String>,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct InviteResponse {
    pub id: Uuid,
    pub code: String,
    pub created_by: Uuid,
    pub label: Option<String>,
    pub max_uses: Option<i32>,
    pub use_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<InviteRow> for InviteResponse {
    fn from(row: InviteRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            created_by: row.created_by,
            label: row.label,
            max_uses: row.max_uses,
            use_count: row.use_count,
            expires_at: row.expires_at,
            revoked_at: row.revoked_at,
            created_at: row.created_at,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateInviteRequest {
    pub label: Option<String>,
    pub max_uses: Option<i32>,
    pub expires_in_hours: Option<i64>,
}
