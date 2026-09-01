// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Serialize)]
pub struct InterventionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub administered_at: DateTime<Utc>,
    pub fasted: Option<bool>,
    pub timing_relative_to: Option<String>,
    pub notes: Option<String>,
    pub healthkit_written: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateIntervention {
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub administered_at: DateTime<Utc>,
    pub fasted: Option<bool>,
    pub timing_relative_to: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct InterventionQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// PATCH /interventions/:id — all fields optional, unset fields are left
/// unchanged. No substance-name validation per project rules.
#[derive(Deserialize)]
pub struct UpdateIntervention {
    pub substance: Option<String>,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub administered_at: Option<DateTime<Utc>>,
    pub fasted: Option<bool>,
    pub timing_relative_to: Option<String>,
    pub notes: Option<String>,
}
