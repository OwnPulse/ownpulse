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
pub mod models;
pub mod routes;
pub mod stats;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use axum::{routing::get, Json, Router};
use axum_prometheus::PrometheusMetricLayer;
use config::Config;
use serde_json::json;
use sqlx::PgPool;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};

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
pub fn build_app(state: AppState) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1", routes::api_routes())
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .layer(cors_layer(&state.config.web_origin))
        .with_state(state)
}

/// Build the application router without Prometheus metrics.
/// Used by integration tests to avoid global recorder conflicts across
/// parallel test threads.
pub fn build_app_without_metrics(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1", routes::api_routes())
        .layer(cors_layer(&state.config.web_origin))
        .with_state(state)
}
