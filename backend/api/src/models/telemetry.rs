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
    /// Originating platform: `"ios"` or `"web"`. Defaults to `"ios"` for
    /// backward compatibility with older iOS clients that omit it.
    pub platform: Option<String>,
}

/// Platforms permitted to submit telemetry. Anything else falls back to the
/// default so the `platform` column stays a small, known set of values.
const VALID_PLATFORMS: &[&str] = &["ios", "web"];

pub fn is_valid_platform(p: &str) -> bool {
    VALID_PLATFORMS.contains(&p)
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

const VALID_EVENT_TYPES: &[&str] = &["crash", "screen", "flow", "api_call"];

pub fn is_valid_event_type(t: &str) -> bool {
    VALID_EVENT_TYPES.contains(&t)
}

/// Fields permitted in an `api_call` telemetry payload. Anything else is
/// stripped before storage so no request/response bodies, path-segment IDs,
/// or other potentially-identifying values leak into telemetry.
///
/// Both `status`/`status_code` and `latency`/`latency_ms` spellings are
/// accepted because iOS and web clients differ.
const API_CALL_ALLOWED_FIELDS: &[&str] = &[
    "endpoint",
    "method",
    "status",
    "status_code",
    "latency",
    "latency_ms",
    "retry_count",
];

/// Normalize an endpoint path so it carries no identifying path-segment
/// values. Drops any query string or fragment and replaces segments that look
/// like identifiers (all-digit, or hyphenated-with-digit / UUID-shaped) with
/// `:id`. So `/protocols/42/runs/9c1f-…?token=x` becomes `/protocols/:id/runs/:id`.
pub fn normalize_endpoint(endpoint: &str) -> String {
    let path = endpoint.split(['?', '#']).next().unwrap_or(endpoint);
    if path.is_empty() {
        return "unknown".to_string();
    }
    path.split('/')
        .map(|seg| {
            if seg.is_empty() {
                seg.to_string()
            } else if segment_looks_like_id(seg) {
                ":id".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A path segment is treated as an identifier if it is all digits, or contains
/// a hyphen alongside a digit (UUID-ish / slugged IDs).
fn segment_looks_like_id(seg: &str) -> bool {
    let all_digits = seg.chars().all(|c| c.is_ascii_digit());
    let uuid_ish = seg.contains('-') && seg.chars().any(|c| c.is_ascii_digit());
    all_digits || uuid_ish
}

/// Return a new payload containing only the allowlisted `api_call` fields.
/// Drops everything else, and normalizes the `endpoint` value so no
/// path-segment identifiers are persisted. If the payload is not a JSON
/// object, returns an empty object.
pub fn scrub_api_call_payload(payload: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            if !API_CALL_ALLOWED_FIELDS.contains(&k.as_str()) {
                continue;
            }
            match (k.as_str(), v.as_str()) {
                ("endpoint", Some(ep)) => {
                    out.insert(k.clone(), serde_json::Value::String(normalize_endpoint(ep)));
                }
                _ => {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }
    serde_json::Value::Object(out)
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
        assert!(is_valid_event_type("api_call"));
        assert!(!is_valid_event_type("login"));
        assert!(!is_valid_event_type(""));
    }

    #[test]
    fn valid_platforms() {
        assert!(is_valid_platform("ios"));
        assert!(is_valid_platform("web"));
        assert!(!is_valid_platform("android"));
        assert!(!is_valid_platform(""));
    }

    #[test]
    fn normalize_endpoint_strips_ids_and_query() {
        assert_eq!(
            normalize_endpoint("/protocols/42/runs"),
            "/protocols/:id/runs"
        );
        assert_eq!(
            normalize_endpoint("/users/9c1f2e3d-abcd/profile?token=x"),
            "/users/:id/profile"
        );
        assert_eq!(normalize_endpoint("/account"), "/account");
        assert_eq!(normalize_endpoint(""), "unknown");
    }

    #[test]
    fn scrub_api_call_normalizes_endpoint() {
        let payload = serde_json::json!({"endpoint": "/protocols/42/runs", "method": "GET"});
        let scrubbed = scrub_api_call_payload(&payload);
        assert_eq!(
            scrubbed.get("endpoint").and_then(|v| v.as_str()),
            Some("/protocols/:id/runs")
        );
    }

    #[test]
    fn scrub_api_call_keeps_only_allowlisted_fields() {
        let payload = serde_json::json!({
            "endpoint": "/health_records",
            "method": "POST",
            "status": 201,
            "status_code": 201,
            "latency": 42,
            "latency_ms": 42,
            "retry_count": 1,
            // Disallowed fields that must be stripped:
            "request_body": {"value": 99},
            "response_body": "ok",
            "user_id": "abc-123",
            "auth_token": "secret",
        });
        let scrubbed = scrub_api_call_payload(&payload);
        let obj = scrubbed.as_object().unwrap();
        assert_eq!(obj.len(), 7);
        assert!(obj.contains_key("endpoint"));
        assert!(obj.contains_key("method"));
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("status_code"));
        assert!(obj.contains_key("latency"));
        assert!(obj.contains_key("latency_ms"));
        assert!(obj.contains_key("retry_count"));
        assert!(!obj.contains_key("request_body"));
        assert!(!obj.contains_key("response_body"));
        assert!(!obj.contains_key("user_id"));
        assert!(!obj.contains_key("auth_token"));
    }

    #[test]
    fn scrub_api_call_non_object_yields_empty() {
        let scrubbed = scrub_api_call_payload(&serde_json::json!("not an object"));
        assert_eq!(scrubbed, serde_json::json!({}));
    }

    #[test]
    fn deserialize_report() {
        let json = r#"{"events":[{"type":"screen","payload":{"screen":"dashboard"},"app_version":"1.0.0"}]}"#;
        let report: TelemetryReport = serde_json::from_str(json).unwrap();
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].event_type, "screen");
    }
}
