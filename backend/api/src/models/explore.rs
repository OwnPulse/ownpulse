// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Source-field enum allowlist — CRITICAL for security.
// No user input is ever interpolated into SQL. All field names come from these
// enums, and lab markers are always bind-parameterized.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MetricSource {
    HealthRecord(HealthRecordField),
    Checkin(CheckinField),
    Lab(String),
    Calendar(CalendarField),
    Sleep(SleepField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthRecordField {
    HeartRate,
    HeartRateVariability,
    RestingHeartRate,
    BodyMass,
    BodyFatPercentage,
    BodyTemperature,
    BloodPressureSystolic,
    BloodPressureDiastolic,
    BloodGlucose,
    BloodOxygen,
    RespiratoryRate,
    Steps,
    ActiveEnergy,
    BasalEnergy,
    Vo2Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckinField {
    Energy,
    Mood,
    Focus,
    Recovery,
    Libido,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarField {
    MeetingMinutes,
    MeetingCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepField {
    DurationMinutes,
    DeepMinutes,
    RemMinutes,
    Score,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Daily,
    Weekly,
    Monthly,
}

impl Resolution {
    /// Return the PostgreSQL `date_trunc` interval string.
    pub fn pg_interval(&self) -> &'static str {
        match self {
            Resolution::Daily => "day",
            Resolution::Weekly => "week",
            Resolution::Monthly => "month",
        }
    }
}

impl HealthRecordField {
    /// The `record_type` string stored in `health_records.record_type`.
    pub fn record_type(&self) -> &'static str {
        match self {
            Self::HeartRate => "heart_rate",
            Self::HeartRateVariability => "heart_rate_variability",
            Self::RestingHeartRate => "resting_heart_rate",
            Self::BodyMass => "body_mass",
            Self::BodyFatPercentage => "body_fat_percentage",
            Self::BodyTemperature => "body_temperature",
            Self::BloodPressureSystolic => "blood_pressure_systolic",
            Self::BloodPressureDiastolic => "blood_pressure_diastolic",
            Self::BloodGlucose => "blood_glucose",
            Self::BloodOxygen => "blood_oxygen",
            Self::RespiratoryRate => "respiratory_rate",
            Self::Steps => "steps",
            Self::ActiveEnergy => "active_energy",
            Self::BasalEnergy => "basal_energy",
            Self::Vo2Max => "vo2_max",
        }
    }

    /// Human-readable unit for this metric.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::HeartRate | Self::RestingHeartRate => "bpm",
            Self::HeartRateVariability => "ms",
            Self::BodyMass => "kg",
            Self::BodyFatPercentage => "%",
            Self::BodyTemperature => "°C",
            Self::BloodPressureSystolic | Self::BloodPressureDiastolic => "mmHg",
            Self::BloodGlucose => "mg/dL",
            Self::BloodOxygen => "%",
            Self::RespiratoryRate => "breaths/min",
            Self::Steps => "steps",
            Self::ActiveEnergy | Self::BasalEnergy => "kcal",
            Self::Vo2Max => "mL/kg/min",
        }
    }

    /// Parse a field name string into a `HealthRecordField`.
    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "heart_rate" => Some(Self::HeartRate),
            "heart_rate_variability" => Some(Self::HeartRateVariability),
            "resting_heart_rate" => Some(Self::RestingHeartRate),
            "body_mass" => Some(Self::BodyMass),
            "body_fat_percentage" => Some(Self::BodyFatPercentage),
            "body_temperature" => Some(Self::BodyTemperature),
            "blood_pressure_systolic" => Some(Self::BloodPressureSystolic),
            "blood_pressure_diastolic" => Some(Self::BloodPressureDiastolic),
            "blood_glucose" => Some(Self::BloodGlucose),
            "blood_oxygen" => Some(Self::BloodOxygen),
            "respiratory_rate" => Some(Self::RespiratoryRate),
            "steps" => Some(Self::Steps),
            "active_energy" => Some(Self::ActiveEnergy),
            "basal_energy" => Some(Self::BasalEnergy),
            "vo2_max" => Some(Self::Vo2Max),
            _ => None,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::HeartRate => "Heart Rate",
            Self::HeartRateVariability => "Heart Rate Variability",
            Self::RestingHeartRate => "Resting Heart Rate",
            Self::BodyMass => "Body Mass",
            Self::BodyFatPercentage => "Body Fat %",
            Self::BodyTemperature => "Body Temperature",
            Self::BloodPressureSystolic => "Blood Pressure (Systolic)",
            Self::BloodPressureDiastolic => "Blood Pressure (Diastolic)",
            Self::BloodGlucose => "Blood Glucose",
            Self::BloodOxygen => "Blood Oxygen",
            Self::RespiratoryRate => "Respiratory Rate",
            Self::Steps => "Steps",
            Self::ActiveEnergy => "Active Energy",
            Self::BasalEnergy => "Basal Energy",
            Self::Vo2Max => "VO2 Max",
        }
    }

    /// All variants, for building the static metric list.
    pub fn all() -> &'static [Self] {
        &[
            Self::HeartRate,
            Self::HeartRateVariability,
            Self::RestingHeartRate,
            Self::BodyMass,
            Self::BodyFatPercentage,
            Self::BodyTemperature,
            Self::BloodPressureSystolic,
            Self::BloodPressureDiastolic,
            Self::BloodGlucose,
            Self::BloodOxygen,
            Self::RespiratoryRate,
            Self::Steps,
            Self::ActiveEnergy,
            Self::BasalEnergy,
            Self::Vo2Max,
        ]
    }
}

impl CheckinField {
    /// Column name in the `daily_checkins` table.
    pub fn column(&self) -> &'static str {
        match self {
            Self::Energy => "energy",
            Self::Mood => "mood",
            Self::Focus => "focus",
            Self::Recovery => "recovery",
            Self::Libido => "libido",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "energy" => Some(Self::Energy),
            "mood" => Some(Self::Mood),
            "focus" => Some(Self::Focus),
            "recovery" => Some(Self::Recovery),
            "libido" => Some(Self::Libido),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Energy => "Energy",
            Self::Mood => "Mood",
            Self::Focus => "Focus",
            Self::Recovery => "Recovery",
            Self::Libido => "Libido",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Energy,
            Self::Mood,
            Self::Focus,
            Self::Recovery,
            Self::Libido,
        ]
    }
}

impl CalendarField {
    pub fn column(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "meeting_minutes",
            Self::MeetingCount => "meeting_count",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "meeting_minutes" => Some(Self::MeetingMinutes),
            "meeting_count" => Some(Self::MeetingCount),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "Meeting Minutes",
            Self::MeetingCount => "Meeting Count",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "min",
            Self::MeetingCount => "count",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::MeetingMinutes, Self::MeetingCount]
    }
}

impl SleepField {
    /// JSONB key in `observations.value` for sleep records.
    pub fn json_key(&self) -> &'static str {
        match self {
            Self::DurationMinutes => "duration_minutes",
            Self::DeepMinutes => "deep_minutes",
            Self::RemMinutes => "rem_minutes",
            Self::Score => "score",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "duration_minutes" => Some(Self::DurationMinutes),
            "deep_minutes" => Some(Self::DeepMinutes),
            "rem_minutes" => Some(Self::RemMinutes),
            "score" => Some(Self::Score),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::DurationMinutes => "Sleep Duration",
            Self::DeepMinutes => "Deep Sleep",
            Self::RemMinutes => "REM Sleep",
            Self::Score => "Sleep Score",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::DurationMinutes | Self::DeepMinutes | Self::RemMinutes => "min",
            Self::Score => "score",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::DurationMinutes,
            Self::DeepMinutes,
            Self::RemMinutes,
            Self::Score,
        ]
    }
}

impl MetricSource {
    /// Parse a (source, field) pair into a validated `MetricSource`.
    /// Returns `ApiError::BadRequest` if the source or field is not in the allowlist.
    pub fn parse(source: &str, field: &str) -> Result<Self, ApiError> {
        match source {
            "health_records" => HealthRecordField::parse(field)
                .map(MetricSource::HealthRecord)
                .ok_or_else(|| {
                    ApiError::BadRequest(format!("invalid health_records field: {field}"))
                }),
            "checkins" => CheckinField::parse(field)
                .map(MetricSource::Checkin)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid checkins field: {field}"))),
            "labs" => {
                if field.is_empty() {
                    Err(ApiError::BadRequest(
                        "labs field (marker name) must not be empty".to_string(),
                    ))
                } else {
                    Ok(MetricSource::Lab(field.to_string()))
                }
            }
            "calendar" => CalendarField::parse(field)
                .map(MetricSource::Calendar)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid calendar field: {field}"))),
            "sleep" => SleepField::parse(field)
                .map(MetricSource::Sleep)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid sleep field: {field}"))),
            _ => Err(ApiError::BadRequest(format!("invalid source: {source}"))),
        }
    }

    /// Return the unit string for this metric.
    pub fn unit(&self) -> String {
        match self {
            MetricSource::HealthRecord(f) => f.unit().to_string(),
            MetricSource::Checkin(_) => "score".to_string(),
            MetricSource::Lab(_) => "value".to_string(),
            MetricSource::Calendar(f) => f.unit().to_string(),
            MetricSource::Sleep(f) => f.unit().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SeriesResponse {
    pub source: String,
    pub field: String,
    pub unit: String,
    pub points: Vec<DataPoint>,
}

#[derive(Debug, Serialize)]
pub struct DataPoint {
    pub t: DateTime<Utc>,
    pub v: f64,
    pub n: i64,
}

#[derive(Debug, Deserialize)]
pub struct SeriesQuery {
    pub source: String,
    pub field: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
}

#[derive(Debug, Deserialize)]
pub struct BatchSeriesRequest {
    pub metrics: Vec<MetricSpec>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
}

#[derive(Debug, Deserialize)]
pub struct MetricSpec {
    pub source: String,
    pub field: String,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub sources: Vec<MetricSourceGroup>,
}

#[derive(Debug, Serialize)]
pub struct MetricSourceGroup {
    pub source: String,
    pub label: String,
    pub metrics: Vec<MetricOption>,
}

#[derive(Debug, Serialize)]
pub struct MetricOption {
    pub field: String,
    pub label: String,
    pub unit: String,
}

// ---------------------------------------------------------------------------
// Saved chart types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateChart {
    pub name: String,
    pub config: ChartConfig,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChart {
    pub name: Option<String>,
    pub config: Option<ChartConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartConfig {
    pub version: u8,
    pub metrics: Vec<ChartMetricConfig>,
    pub range: ChartRange,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartMetricConfig {
    pub source: String,
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ChartRange {
    Preset { preset: String },
    Custom { start: String, end: String },
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChartRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Validate chart config: version, metric count, metric fields, range, colors.
pub fn validate_chart_config(config: &ChartConfig) -> Result<(), ApiError> {
    if config.version != 1 {
        return Err(ApiError::BadRequest(format!(
            "unsupported chart config version: {}",
            config.version
        )));
    }

    if config.metrics.is_empty() {
        return Err(ApiError::BadRequest(
            "chart must have at least one metric".to_string(),
        ));
    }

    if config.metrics.len() > 8 {
        return Err(ApiError::BadRequest(
            "chart may have at most 8 metrics".to_string(),
        ));
    }

    for m in &config.metrics {
        MetricSource::parse(&m.source, &m.field)?;

        if let Some(ref color) = m.color
            && !is_valid_hex_color(color)
        {
            return Err(ApiError::BadRequest(format!(
                "invalid color: {color} (expected #rrggbb)"
            )));
        }
    }

    match &config.range {
        ChartRange::Preset { preset } => {
            if !["7d", "30d", "90d", "1y", "all"].contains(&preset.as_str()) {
                return Err(ApiError::BadRequest(format!(
                    "invalid range preset: {preset}"
                )));
            }
        }
        ChartRange::Custom { start, end } => {
            // Validate that start and end are parseable dates
            chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| {
                ApiError::BadRequest(format!("invalid custom range start date: {start}"))
            })?;
            chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| {
                ApiError::BadRequest(format!("invalid custom range end date: {end}"))
            })?;
        }
    }

    Ok(())
}

fn is_valid_hex_color(s: &str) -> bool {
    s.len() == 7
        && s.starts_with('#')
        && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// SSE event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct DataChangedEvent {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_health_record_fields() {
        for field in HealthRecordField::all() {
            let result = MetricSource::parse("health_records", field.record_type());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_checkin_fields() {
        for field in CheckinField::all() {
            let result = MetricSource::parse("checkins", field.column());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_calendar_fields() {
        for field in CalendarField::all() {
            let result = MetricSource::parse("calendar", field.column());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_sleep_fields() {
        for field in SleepField::all() {
            let result = MetricSource::parse("sleep", field.json_key());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_lab_field() {
        let result = MetricSource::parse("labs", "testosterone");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_invalid_source() {
        let result = MetricSource::parse("invalid_source", "heart_rate");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_health_record_field() {
        let result = MetricSource::parse("health_records", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_checkin_field() {
        let result = MetricSource::parse("checkins", "happiness");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_lab_marker() {
        let result = MetricSource::parse("labs", "");
        assert!(result.is_err());
    }

    #[test]
    fn resolution_serde_roundtrip() {
        let json = serde_json::to_string(&Resolution::Daily).unwrap();
        assert_eq!(json, r#""daily""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Daily);

        let json = serde_json::to_string(&Resolution::Weekly).unwrap();
        assert_eq!(json, r#""weekly""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Weekly);

        let json = serde_json::to_string(&Resolution::Monthly).unwrap();
        assert_eq!(json, r#""monthly""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Monthly);
    }

    #[test]
    fn resolution_pg_interval() {
        assert_eq!(Resolution::Daily.pg_interval(), "day");
        assert_eq!(Resolution::Weekly.pg_interval(), "week");
        assert_eq!(Resolution::Monthly.pg_interval(), "month");
    }

    #[test]
    fn valid_chart_config() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: Some("#ff0000".to_string()),
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_ok());
    }

    #[test]
    fn invalid_chart_version() {
        let config = ChartConfig {
            version: 2,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_empty_metrics() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_too_many_metrics() {
        let metrics: Vec<ChartMetricConfig> = (0..9)
            .map(|_| ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            })
            .collect();
        let config = ChartConfig {
            version: 1,
            metrics,
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_bad_preset() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Preset {
                preset: "5d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_bad_color() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: Some("red".to_string()),
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn valid_chart_custom_range() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "health_records".to_string(),
                field: "heart_rate".to_string(),
                color: None,
            }],
            range: ChartRange::Custom {
                start: "2026-01-01".to_string(),
                end: "2026-03-01".to_string(),
            },
            resolution: Resolution::Weekly,
        };
        assert!(validate_chart_config(&config).is_ok());
    }

    #[test]
    fn invalid_chart_bad_custom_date() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Custom {
                start: "not-a-date".to_string(),
                end: "2026-03-01".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn hex_color_validation() {
        assert!(is_valid_hex_color("#ff0000"));
        assert!(is_valid_hex_color("#AABBCC"));
        assert!(!is_valid_hex_color("ff0000"));
        assert!(!is_valid_hex_color("#fff"));
        assert!(!is_valid_hex_color("#gggggg"));
    }
}
