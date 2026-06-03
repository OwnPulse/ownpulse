// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use tracing::warn;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::telemetry as db_telemetry;
use crate::models::telemetry::{
    TelemetryEvent, TelemetryReport, TelemetryResponse, contains_health_data, is_valid_event_type,
    sanitize_device_id, scrub_api_call_payload, version_label,
};

/// Resolve the platform for an event, defaulting to `"ios"` for backward
/// compatibility and ignoring any unrecognized value.
fn resolved_platform(event: &TelemetryEvent) -> &'static str {
    match event.platform.as_deref() {
        Some("web") => "web",
        // Any other recognized platform, or an unrecognized/absent one,
        // defaults to "ios" for backward compatibility.
        _ => "ios",
    }
}

const MAX_BATCH_SIZE: usize = 50;

/// POST /telemetry/report — receive anonymous crash reports and flow events.
///
/// Requires JWT authentication (to prevent abuse) but deliberately discards the
/// user identity before storage. Crash and `api_call` events are persisted;
/// screen/flow events only increment Prometheus counters. `api_call` payloads
/// are stripped to an allowlist of non-identifying fields, each coerced to its
/// expected scalar type, before storage; `api_call` rows carry no device_id.
pub async fn report(
    State(state): State<AppState>,
    AuthUser { .. }: AuthUser, // JWT gate only — user_id intentionally ignored
    Json(body): Json<TelemetryReport>,
) -> Result<Json<TelemetryResponse>, (StatusCode, String)> {
    if body.events.is_empty() {
        return Ok(Json(TelemetryResponse {
            accepted: 0,
            rejected: 0,
        }));
    }

    if body.events.len() > MAX_BATCH_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("at most {MAX_BATCH_SIZE} events per request"),
        ));
    }

    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for event in &body.events {
        if !is_valid_event_type(&event.event_type) {
            rejected += 1;
            continue;
        }

        if contains_health_data(&event.payload) {
            warn!(
                event_type = %event.event_type,
                "rejected telemetry event containing health data"
            );
            rejected += 1;
            continue;
        }

        match event.event_type.as_str() {
            "crash" => {
                // Persist crash events for debugging. The device_id is bounded
                // to an opaque-token shape so it can't smuggle free-text PII.
                let pool = state.pool.clone();
                let event_type = event.event_type.clone();
                let device_id = sanitize_device_id(event.device_id.as_deref());
                let payload = event.payload.clone();
                let app_version = event.app_version.clone();
                let platform = resolved_platform(event);

                tokio::spawn(async move {
                    if let Err(e) = db_telemetry::insert_event(
                        &pool,
                        &event_type,
                        device_id.as_deref(),
                        &payload,
                        app_version.as_deref(),
                        platform,
                    )
                    .await
                    {
                        warn!(error = %e, "failed to insert crash event");
                    }
                });

                increment_crash_counter(event);
            }
            "screen" => {
                increment_screen_counter(event);
            }
            "flow" => {
                increment_flow_counter(event);
            }
            "api_call" => {
                // Strip the payload down to the allowlisted, type-coerced fields
                // before it ever touches storage or metrics — no request/response
                // bodies, no path identifiers, no auth material.
                let scrubbed = scrub_api_call_payload(&event.payload);
                let platform = resolved_platform(event);

                // Data minimization: api_call rows are NOT associated with a
                // device_id. The scrubbed fields are fully captured by the
                // Prometheus aggregates; persisting device_id per call would let
                // someone reconstruct a per-device behavioral trace. We store the
                // row (bounded scalar payload + platform) but never the device.
                let pool = state.pool.clone();
                let event_type = event.event_type.clone();
                let payload = scrubbed.clone();
                let app_version = event.app_version.clone();

                tokio::spawn(async move {
                    if let Err(e) = db_telemetry::insert_event(
                        &pool,
                        &event_type,
                        None, // device_id intentionally not stored for api_call
                        &payload,
                        app_version.as_deref(),
                        platform,
                    )
                    .await
                    {
                        warn!(error = %e, "failed to insert api_call event");
                    }
                });

                increment_api_call_counter(event, &scrubbed, platform);
            }
            _ => {
                rejected += 1;
                continue;
            }
        }

        accepted += 1;
    }

    Ok(Json(TelemetryResponse { accepted, rejected }))
}

fn increment_crash_counter(event: &TelemetryEvent) {
    let signal = event
        .payload
        .get("signal")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = event.app_version.as_deref().unwrap_or("unknown");
    metrics::counter!(
        "ownpulse_app_crash_total",
        "signal" => signal.to_string(),
        "version" => version.to_string()
    )
    .increment(1);
}

fn increment_screen_counter(event: &TelemetryEvent) {
    let screen = event
        .payload
        .get("screen")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = event.app_version.as_deref().unwrap_or("unknown");
    metrics::counter!(
        "ownpulse_app_screen_view_total",
        "screen" => screen.to_string(),
        "version" => version.to_string()
    )
    .increment(1);
}

/// Map an HTTP status code to a coarse status class label (`2xx`, `4xx`, …).
fn status_class(status: i64) -> &'static str {
    match status {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "unknown",
    }
}

fn increment_api_call_counter(
    event: &TelemetryEvent,
    payload: &serde_json::Value,
    platform: &'static str,
) {
    // `payload` is the already-scrubbed payload, so `endpoint` is already
    // normalized (path-segment IDs collapsed to `:id`) — use it as the label
    // directly to keep Prometheus cardinality bounded.
    let endpoint = payload
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Accept both `status` and `status_code` spellings.
    let status = payload
        .get("status")
        .or_else(|| payload.get("status_code"))
        .and_then(|v| v.as_i64());
    let class = status.map(status_class).unwrap_or("unknown");

    // Bound the version label: an unbounded client-supplied version is a
    // cardinality-explosion vector for the metrics registry. Only a strict
    // release-version shape is echoed; anything else becomes "unknown".
    let version = version_label(event.app_version.as_deref());

    metrics::counter!(
        "ownpulse_app_api_call_total",
        "platform" => platform,
        "endpoint" => endpoint,
        "status_class" => class,
        "version" => version.to_string()
    )
    .increment(1);

    // Accept both `latency` and `latency_ms` spellings.
    if let Some(latency) = payload
        .get("latency_ms")
        .or_else(|| payload.get("latency"))
        .and_then(|v| v.as_f64())
    {
        metrics::histogram!("ownpulse_app_api_call_latency_ms").record(latency);
    }
}

fn increment_flow_counter(event: &TelemetryEvent) {
    let flow = event
        .payload
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let outcome = event
        .payload
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let version = event.app_version.as_deref().unwrap_or("unknown");
    metrics::counter!(
        "ownpulse_app_flow_total",
        "flow" => flow.to_string(),
        "outcome" => outcome.to_string(),
        "version" => version.to_string()
    )
    .increment(1);
}
