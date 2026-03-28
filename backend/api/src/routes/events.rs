// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::jwt::decode_access_token;
use crate::db::users;
use crate::error::ApiError;
use crate::models::explore::DataChangedEvent;

#[derive(Deserialize)]
pub struct EventsQuery {
    pub token: String,
}

/// GET /events?token=<JWT> — SSE endpoint for real-time data change events.
///
/// Uses query param auth because `EventSource` does not support custom headers.
pub async fn events_stream(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Validate JWT from query param
    let claims =
        decode_access_token(&query.token, &state.config.jwt_secret, &state.config.web_origin)
            .map_err(|_| ApiError::Unauthorized)?;

    // Verify user exists and is active
    let user = users::find_by_id(&state.pool, claims.sub)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    if user.status != "active" {
        return Err(ApiError::Forbidden);
    }

    // NOTE: JWT is passed as a query param because EventSource does not support
    // custom headers. This means the token appears in server access logs. The token
    // is short-lived (1 hour) which limits exposure. A ticket-based exchange is a
    // future improvement (see ADR-0011).
    let user_id = user.id;
    let jwt_secret = state.config.jwt_secret.clone();
    let web_origin = state.config.web_origin.clone();
    let token = query.token.clone();
    let pool = state.pool.clone();
    let mut rx = state.event_tx.subscribe();

    let stream = async_stream::stream! {
        let mut revalidate = tokio::time::interval(Duration::from_secs(300));
        revalidate.tick().await; // consume first immediate tick

        loop {
            tokio::select! {
                _ = revalidate.tick() => {
                    // Re-check token expiry and user status every 5 minutes
                    if decode_access_token(&token, &jwt_secret, &web_origin).is_err() {
                        tracing::info!(user_id = %user_id, "SSE: token expired, closing");
                        break;
                    }
                    if let Ok(u) = users::find_by_id(&pool, user_id).await {
                        if u.status != "active" {
                            tracing::info!(user_id = %user_id, "SSE: user inactive, closing");
                            break;
                        }
                    }
                }
                result = rx.recv() => {
                    match result {
                        Ok((uid, event)) if uid == user_id => {
                            if let Ok(data) = serde_json::to_string(&event) {
                                yield Ok(Event::default().event("data_changed").data(data));
                            }
                        }
                        Ok(_) => {
                            // Event for a different user — skip.
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(user_id = %user_id, lagged = n, "SSE client lagged behind");
                            // Continue receiving.
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keepalive"),
    ))
}

/// Publish a data-changed event to the broadcast channel.
/// Ignoring send errors is intentional — no subscribers means the event is simply dropped.
pub fn publish_event(
    event_tx: &tokio::sync::broadcast::Sender<(Uuid, DataChangedEvent)>,
    user_id: Uuid,
    source: &str,
    record_type: Option<&str>,
) {
    let event = DataChangedEvent {
        source: source.to_string(),
        record_type: record_type.map(|s| s.to_string()),
    };
    let _ = event_tx.send((user_id, event));
}
