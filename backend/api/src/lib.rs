// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod email;
pub mod error;
pub mod export;
pub mod genetics;
pub mod integrations;
pub mod jobs;
pub mod migrate;
pub mod migration_check;
pub mod models;
pub mod routes;
pub mod stats;

use std::sync::atomic::Ordering;

use axum::extract::State;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use config::Config;
use migration_check::MigrationsReady;
use models::explore::DataChangedEvent;
use serde_json::json;
use sqlx::PgPool;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

// --- A3: client bundle version observability ---------------------------------
// Web/iOS clients send their build version (git SHA) as `X-App-Version`. We
// record it as a single span field on every request so Loki can surface which
// client builds are still calling the API (i.e. detect stale clients). No other
// header is logged here, and the value is a build identifier — never user data.
const X_APP_VERSION: HeaderName = HeaderName::from_static("x-app-version");

/// Extract the client build version from request headers, falling back to
/// `"unknown"` when the header is absent or not valid UTF-8.
fn app_version_from_headers(headers: &axum::http::HeaderMap) -> &str {
    headers
        .get(&X_APP_VERSION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
}

/// A [`TraceLayer`] that adds the client `X-App-Version` to each request span.
fn http_trace_layer<B>() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    impl Fn(&Request<B>) -> tracing::Span + Clone,
> {
    TraceLayer::new_for_http().make_span_with(|request: &Request<B>| {
        tracing::info_span!(
            "request",
            method = %request.method(),
            uri = %request.uri(),
            app_version = %app_version_from_headers(request.headers()),
        )
    })
}
// --- end A3 -------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub http_client: reqwest::Client,
    pub migrations_ready: MigrationsReady,
    pub event_tx: tokio::sync::broadcast::Sender<(uuid::Uuid, DataChangedEvent)>,
}

/// Liveness probe — always returns 200 if the process is running.
async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

/// Readiness probe — returns 503 if migrations are behind.
///
/// Kubernetes should use this for the readinessProbe so traffic is not
/// routed to a pod whose database schema is outdated.
async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.migrations_ready.load(Ordering::SeqCst) {
        (StatusCode::OK, Json(json!({"status": "healthy"}))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "reason": "database migrations behind",
                "expected": migration_check::EXPECTED_MIGRATION_COUNT
            })),
        )
            .into_response()
    }
}

fn cors_layer(web_origin: &str) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            web_origin
                .parse::<HeaderValue>()
                .expect("invalid WEB_ORIGIN"),
        )
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::list([
            AUTHORIZATION,
            CONTENT_TYPE,
            X_APP_VERSION,
        ]))
        .allow_credentials(true)
}

/// Build the application router with Prometheus metrics.
///
/// Metrics are served on a **separate** internal listener (port 9090) so they
/// are never exposed through the public ingress.  Call [`spawn_metrics_server`]
/// after building the app to start that listener.
pub fn build_app(state: AppState) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    // Spawn internal metrics server on port 9090
    tokio::spawn(async move {
        let metrics_app = Router::new().route(
            "/metrics",
            get(move || {
                let h = metric_handle.clone();
                async move { h.render() }
            }),
        );

        let listener = tokio::net::TcpListener::bind("0.0.0.0:9090")
            .await
            .expect("failed to bind metrics port 9090");

        info!("metrics listening on 0.0.0.0:9090");

        axum::serve(listener, metrics_app)
            .await
            .expect("metrics server error");
    });

    Router::new()
        .route("/api/v1/health", get(health))
        .route("/readyz", get(readyz))
        .nest("/api/v1", routes::api_routes())
        .layer(http_trace_layer())
        .layer(prometheus_layer)
        .layer(cors_layer(&state.config.web_origin))
        .with_state(state)
}

/// Build the application router without Prometheus metrics or rate limiting.
/// Used by integration tests where `ConnectInfo` is not available and global
/// recorder conflicts would occur across parallel test threads.
pub fn build_app_without_metrics(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/readyz", get(readyz))
        .nest("/api/v1", routes::api_routes_without_rate_limit())
        .layer(http_trace_layer())
        .layer(cors_layer(&state.config.web_origin))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::{X_APP_VERSION, app_version_from_headers};

    #[test]
    fn reads_x_app_version_header() {
        let mut headers = HeaderMap::new();
        headers.insert(X_APP_VERSION, "28c7559".parse().unwrap());
        assert_eq!(app_version_from_headers(&headers), "28c7559");
    }

    #[test]
    fn falls_back_to_unknown_when_header_absent() {
        let headers = HeaderMap::new();
        assert_eq!(app_version_from_headers(&headers), "unknown");
    }

    #[test]
    fn falls_back_to_unknown_for_non_utf8_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            X_APP_VERSION,
            axum::http::HeaderValue::from_bytes(&[0xff, 0xfe]).unwrap(),
        );
        assert_eq!(app_version_from_headers(&headers), "unknown");
    }
}
