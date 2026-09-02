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
    pub source: String,
    pub source_id: Option<String>,
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
    /// Originating system, e.g. "healthkit". Defaults to "manual".
    pub source: Option<String>,
    /// Stable id in the originating system (e.g. the HealthKit dose-event
    /// UUID). When set, (user, source, source_id) is unique and a replayed
    /// create returns the existing row.
    pub source_id: Option<String>,
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
