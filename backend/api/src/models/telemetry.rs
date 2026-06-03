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

/// Bound a client-supplied `device_id` so it can't be used as a free-text
/// smuggling channel for PII. A device id is an opaque client-generated token;
/// we accept only a UUID/hex/base64url-ish shape (ASCII alphanumerics, `-`,
/// `_`) of bounded length. Anything else (emails, sentences, JSON) is dropped
/// to `None`.
pub fn sanitize_device_id(device_id: Option<&str>) -> Option<String> {
    let id = device_id?;
    if (8..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(id.to_string())
    } else {
        None
    }
}

/// Bound a client-supplied app version for use as a Prometheus label. Returns
/// the version verbatim only if it matches the strict release-version pattern;
/// otherwise `"unknown"`. This caps label cardinality (a client cannot send a
/// unique free-text version per request) and blocks free-text smuggling.
pub fn version_label(version: Option<&str>) -> &str {
    match version {
        Some(v) if is_valid_version(v) => v,
        _ => "unknown",
    }
}

/// True if `v` looks like a release version: 1-4 dot-separated numeric
/// components, optionally followed by a `-` and up to 16 `[A-Za-z0-9.]`
/// characters. Length-capped to keep it cheap and bounded.
pub fn is_valid_version(v: &str) -> bool {
    if v.is_empty() || v.len() > 32 {
        return false;
    }
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let components: Vec<&str> = core.split('.').collect();
    if components.is_empty() || components.len() > 4 {
        return false;
    }
    if !components
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit()))
    {
        return false;
    }
    match pre {
        None => true,
        Some(p) => {
            !p.is_empty()
                && p.len() <= 16
                && p.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.')
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TelemetryResponse {
    pub accepted: usize,
    pub rejected: usize,
}

/// Telemetry-ingest pipeline health surface.
///
/// Aggregate-only liveness signal for monitoring the telemetry pipeline. It
/// reports how many `app_events` arrived in the last 5 minutes and how stale the
/// most recent event is. It deliberately exposes **no** user identity, device
/// id, payload, or any health data — only counts and timestamps.
#[derive(Debug, Serialize)]
pub struct TelemetryHealth {
    /// Count of `app_events` rows received in the last 5 minutes.
    pub events_last_5m: i64,
    /// Timestamp of the most recent `app_events` row, or `null` if none exist.
    pub last_event_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Age in seconds of the most recent event, or `null` if no events exist.
    /// Drives the `TelemetryStalled` alert (fires when no events for 30m).
    pub last_event_age_seconds: Option<i64>,
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

/// Normalize an endpoint path so it carries no identifying path-segment
/// values. Drops any query string or fragment, then keeps a segment **only** if
/// it is a recognizable static route word; every other segment is replaced with
/// `:id`. This is an allowlist on shape, not a blocklist: emails, usernames,
/// hex/base64 tokens, and dashless UUIDs all collapse to `:id` because they are
/// not route words. So `/users/alice@example.com/profile` becomes
/// `/users/:id/profile` and `/records/550e8400e29b41d4a716446655440000?t=x`
/// becomes `/records/:id`.
pub fn normalize_endpoint(endpoint: &str) -> String {
    let path = endpoint.split(['?', '#']).next().unwrap_or(endpoint);
    if path.is_empty() {
        return "unknown".to_string();
    }
    path.split('/')
        .map(|seg| {
            if seg.is_empty() || is_route_word(seg) {
                seg.to_string()
            } else {
                ":id".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A path segment is treated as a static route word only if it is short and
/// composed solely of lowercase ASCII letters and underscores (e.g. `users`,
/// `health_records`, `runs`). Anything containing a digit, an uppercase letter,
/// a hyphen, a dot, an `@`, a `%`, or any other character — or anything longer
/// than 24 chars — is treated as a dynamic identifier and collapsed to `:id`.
/// This errs on the side of over-collapsing: a never-before-seen route word
/// would be hidden as `:id`, which is the privacy-safe failure mode.
fn is_route_word(seg: &str) -> bool {
    !seg.is_empty() && seg.len() <= 24 && seg.chars().all(|c| c.is_ascii_lowercase() || c == '_')
}

/// Return a new payload containing only the allowlisted `api_call` fields,
/// each coerced to its expected scalar type. Anything else is dropped:
///
/// - `endpoint` — string only, and normalized so no path identifiers survive.
/// - `method` — string only, uppercased and restricted to known HTTP methods.
/// - `status` / `status_code` — integer only.
/// - `latency` / `latency_ms` — non-negative integer only.
/// - `retry_count` — non-negative integer only.
///
/// A value of the wrong JSON type (object, array, string-where-int-expected,
/// etc.) is dropped entirely — PII cannot ride in on an allowlisted key.
/// A non-object payload yields an empty object.
pub fn scrub_api_call_payload(payload: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let Some(obj) = payload.as_object() else {
        return serde_json::Value::Object(out);
    };

    if let Some(ep) = obj.get("endpoint").and_then(|v| v.as_str()) {
        out.insert(
            "endpoint".to_string(),
            serde_json::Value::String(normalize_endpoint(ep)),
        );
    }
    if let Some(method) = obj
        .get("method")
        .and_then(|v| v.as_str())
        .and_then(normalize_method)
    {
        out.insert(
            "method".to_string(),
            serde_json::Value::String(method.to_string()),
        );
    }
    for key in ["status", "status_code"] {
        if let Some(n) = obj.get(key).and_then(|v| v.as_i64()) {
            out.insert(key.to_string(), serde_json::Value::Number(n.into()));
        }
    }
    for key in ["latency", "latency_ms", "retry_count"] {
        if let Some(n) = obj.get(key).and_then(|v| v.as_i64()).filter(|n| *n >= 0) {
            out.insert(key.to_string(), serde_json::Value::Number(n.into()));
        }
    }
    serde_json::Value::Object(out)
}

/// HTTP methods permitted in an `api_call` payload. Anything else is dropped so
/// the `method` field can't carry free text.
const VALID_HTTP_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/// Uppercase and validate an HTTP method; returns the canonical static form or
/// `None` if it isn't a recognized method.
fn normalize_method(method: &str) -> Option<&'static str> {
    let upper = method.to_ascii_uppercase();
    VALID_HTTP_METHODS.iter().copied().find(|m| *m == upper)
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
    fn normalize_endpoint_strips_numeric_and_query() {
        assert_eq!(
            normalize_endpoint("/protocols/42/runs"),
            "/protocols/:id/runs"
        );
        assert_eq!(
            normalize_endpoint("/users/9c1f2e3d-abcd/profile?token=x"),
            "/users/:id/profile"
        );
        assert_eq!(normalize_endpoint("/account"), "/account");
        assert_eq!(normalize_endpoint("/health_records"), "/health_records");
        assert_eq!(normalize_endpoint(""), "unknown");
    }

    #[test]
    fn normalize_endpoint_collapses_non_route_words() {
        // Email PII must not survive.
        assert_eq!(
            normalize_endpoint("/users/alice@example.com/profile"),
            "/users/:id/profile"
        );
        // Username with a hyphen (no digit) must not survive.
        assert_eq!(
            normalize_endpoint("/users/jane-doe/profile"),
            "/users/:id/profile"
        );
        // Dashless 32-hex UUID must not survive (contains digits).
        assert_eq!(
            normalize_endpoint("/records/550e8400e29b41d4a716446655440000"),
            "/records/:id"
        );
        // Base64url-ish token (uppercase letters) must not survive.
        assert_eq!(
            normalize_endpoint("/invite/YWxpY2VAZXhhbXBsZQ"),
            "/invite/:id"
        );
        // A purely alphabetic but absurdly long segment must not survive.
        let long = "a".repeat(40);
        assert_eq!(
            normalize_endpoint(&format!("/x/{long}")),
            "/x/:id".to_string()
        );
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
    fn scrub_api_call_drops_non_scalar_values_on_allowlisted_keys() {
        // PII tries to ride in via nested objects/arrays on allowlisted keys.
        let payload = serde_json::json!({
            "endpoint": {"path": "/users/alice@example.com"},
            "method": {"x": "GET"},
            "status": "200",            // string, not int → dropped
            "latency_ms": [1, 2, 3],    // array → dropped
            "retry_count": {"n": 1},    // object → dropped
        });
        let scrubbed = scrub_api_call_payload(&payload);
        let obj = scrubbed.as_object().unwrap();
        // Every wrong-typed value is dropped — nothing survives.
        assert!(obj.is_empty(), "expected empty, got: {scrubbed}");
    }

    #[test]
    fn scrub_api_call_coerces_and_validates_scalars() {
        let payload = serde_json::json!({
            "endpoint": "/account",
            "method": "post",          // lowercase → uppercased
            "status": 200,
            "latency_ms": 12,
            "retry_count": -1,         // negative → dropped
        });
        let scrubbed = scrub_api_call_payload(&payload);
        let obj = scrubbed.as_object().unwrap();
        assert_eq!(obj.get("method").and_then(|v| v.as_str()), Some("POST"));
        assert_eq!(obj.get("status").and_then(|v| v.as_i64()), Some(200));
        assert_eq!(obj.get("latency_ms").and_then(|v| v.as_i64()), Some(12));
        assert!(!obj.contains_key("retry_count"));
    }

    #[test]
    fn scrub_api_call_drops_unknown_http_method() {
        let payload = serde_json::json!({"endpoint": "/x", "method": "TRACE-ish junk"});
        let scrubbed = scrub_api_call_payload(&payload);
        assert!(!scrubbed.as_object().unwrap().contains_key("method"));
    }

    #[test]
    fn version_label_bounds_cardinality() {
        assert_eq!(version_label(Some("1.2.3")), "1.2.3");
        assert_eq!(version_label(Some("12.0")), "12.0");
        assert_eq!(version_label(Some("1.2.3-beta1")), "1.2.3-beta1");
        // Unbounded / free-text versions collapse to a single bucket.
        assert_eq!(
            version_label(Some("malicious unique 9f8a7b6c value")),
            "unknown"
        );
        assert_eq!(version_label(Some(&"9".repeat(100))), "unknown");
        assert_eq!(version_label(None), "unknown");
    }

    #[test]
    fn sanitize_device_id_bounds_shape() {
        assert_eq!(
            sanitize_device_id(Some("AB12cd34-ef56_7890")),
            Some("AB12cd34-ef56_7890".to_string())
        );
        // Too short, contains PII / free text, or wrong charset → dropped.
        assert_eq!(sanitize_device_id(Some("short")), None);
        assert_eq!(sanitize_device_id(Some("alice@example.com")), None);
        assert_eq!(
            sanitize_device_id(Some("a sentence with spaces here")),
            None
        );
        assert_eq!(sanitize_device_id(None), None);
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
