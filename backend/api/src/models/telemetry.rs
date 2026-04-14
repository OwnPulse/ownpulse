// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TelemetryReport {
    pub events: Vec<TelemetryEvent>,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub device_id: Option<String>,
    pub payload: serde_json::Value,
    pub app_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TelemetryResponse {
    pub accepted: usize,
    pub rejected: usize,
}

/// Reject payloads that contain health-related data.
/// Telemetry must never include health information.
const HEALTH_KEYWORDS: &[&str] = &[
    "heart_rate",
    "blood",
    "glucose",
    "sleep",
    "weight",
    "body_mass",
    "medication",
    "substance",
    "dose",
    "intervention",
    "checkin",
    "lab_result",
    "genetic",
];

pub fn contains_health_data(payload: &serde_json::Value) -> bool {
    let s = payload.to_string().to_lowercase();
    HEALTH_KEYWORDS.iter().any(|kw| s.contains(kw))
}

const VALID_EVENT_TYPES: &[&str] = &["crash", "screen", "flow"];

pub fn is_valid_event_type(t: &str) -> bool {
    VALID_EVENT_TYPES.contains(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_health_keywords() {
        let payload = serde_json::json!({"signal": "11", "note": "heart_rate was high"});
        assert!(contains_health_data(&payload));
    }

    #[test]
    fn accepts_clean_payload() {
        let payload = serde_json::json!({"screen": "explore", "outcome": "completed"});
        assert!(!contains_health_data(&payload));
    }

    #[test]
    fn rejects_nested_health_data() {
        let payload = serde_json::json!({"error": {"detail": "failed to sync blood_glucose"}});
        assert!(contains_health_data(&payload));
    }

    #[test]
    fn valid_event_types() {
        assert!(is_valid_event_type("crash"));
        assert!(is_valid_event_type("screen"));
        assert!(is_valid_event_type("flow"));
        assert!(!is_valid_event_type("login"));
        assert!(!is_valid_event_type(""));
    }

    #[test]
    fn deserialize_report() {
        let json = r#"{"events":[{"type":"screen","payload":{"screen":"dashboard"},"app_version":"1.0.0"}]}"#;
        let report: TelemetryReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].event_type, "screen");
    }
}
