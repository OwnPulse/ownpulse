// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod error;
pub mod export;
pub mod integrations;
pub mod jobs;
pub mod migrate;
pub mod models;
pub mod routes;
pub mod stats;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use axum::{Json, Router, routing::get};
use axum_prometheus::PrometheusMetricLayer;
use config::Config;
use serde_json::json;
use sqlx::PgPool;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub http_client: reqwest::Client,
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
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
        .allow_headers(AllowHeaders::list([AUTHORIZATION, CONTENT_TYPE]))
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
        .nest("/api/v1", routes::api_routes())
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
        .nest("/api/v1", routes::api_routes_without_rate_limit())
        .layer(cors_layer(&state.config.web_origin))
        .with_state(state)
}
