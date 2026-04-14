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
};

const MAX_BATCH_SIZE: usize = 50;

/// POST /telemetry/report — receive anonymous crash reports and flow events.
///
/// Requires JWT authentication (to prevent abuse) but deliberately discards the
/// user identity before storage. Crash events are persisted; screen/flow events
/// only increment Prometheus counters.
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
                // Persist crash events for debugging
                let pool = state.pool.clone();
                let event_type = event.event_type.clone();
                let device_id = event.device_id.clone();
                let payload = event.payload.clone();
                let app_version = event.app_version.clone();

                tokio::spawn(async move {
                    if let Err(e) = db_telemetry::insert_event(
                        &pool,
                        &event_type,
                        device_id.as_deref(),
                        &payload,
                        app_version.as_deref(),
                        "ios",
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
