// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Request and response types for the correlation explorer endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::explore::{MetricSource, Resolution};

// ---------------------------------------------------------------------------
// Shared metric spec (used in request bodies)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MetricRef {
    pub source: String,
    pub field: String,
}

impl MetricRef {
    pub fn parse(&self) -> Result<MetricSource, ApiError> {
        MetricSource::parse(&self.source, &self.field)
    }
}

#[derive(Debug, Serialize)]
pub struct MetricRefOut {
    pub source: String,
    pub field: String,
}

// ---------------------------------------------------------------------------
// Before/After
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BeforeAfterRequest {
    pub intervention_substance: String,
    pub metric: MetricRef,
    pub before_days: i64,
    pub after_days: i64,
    pub resolution: Resolution,
}

#[derive(Debug, Serialize)]
pub struct BeforeAfterResponse {
    pub intervention_substance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_dose: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_dose: Option<DateTime<Utc>>,
    pub metric: MetricRefOut,
    pub before: WindowStats,
    pub after: WindowStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value: Option<f64>,
    pub significant: bool,
    pub test_used: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WindowStats {
    pub mean: Option<f64>,
    pub std_dev: Option<f64>,
    pub n: usize,
    pub points: Vec<TimeValue>,
}

#[derive(Debug, Serialize)]
pub struct TimeValue {
    pub t: DateTime<Utc>,
    pub v: f64,
}

// ---------------------------------------------------------------------------
// Correlate
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CorrelateRequest {
    pub metric_a: MetricRef,
    pub metric_b: MetricRef,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
    #[serde(default = "default_method")]
    pub method: CorrelationMethod,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CorrelationMethod {
    Pearson,
    Spearman,
}

fn default_method() -> CorrelationMethod {
    CorrelationMethod::Pearson
}

#[derive(Debug, Serialize)]
pub struct CorrelateResponse {
    pub metric_a: MetricRefOut,
    pub metric_b: MetricRefOut,
    pub r: Option<f64>,
    pub p_value: Option<f64>,
    pub n: usize,
    pub significant: bool,
    pub method: CorrelationMethod,
    pub interpretation: String,
    pub scatter: Vec<ScatterPoint>,
}

#[derive(Debug, Serialize)]
pub struct ScatterPoint {
    pub a: f64,
    pub b: f64,
    pub t: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Lag-Correlate
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LagCorrelateRequest {
    pub metric_a: MetricRef,
    pub metric_b: MetricRef,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
    pub max_lag_days: i64,
    #[serde(default = "default_method")]
    pub method: CorrelationMethod,
}

#[derive(Debug, Serialize)]
pub struct LagCorrelateResponse {
    pub metric_a: MetricRefOut,
    pub metric_b: MetricRefOut,
    pub lags: Vec<LagResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag: Option<BestLag>,
    pub method: CorrelationMethod,
}

#[derive(Debug, Serialize)]
pub struct LagResult {
    pub lag: i64,
    pub r: Option<f64>,
    pub p_value: Option<f64>,
    pub n: usize,
}

#[derive(Debug, Serialize)]
pub struct BestLag {
    pub lag: i64,
    pub r: f64,
    pub p_value: Option<f64>,
}
