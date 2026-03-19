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

use axum::http::HeaderValue;
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

/// Build the application router. Extracted so integration tests can reuse it.
pub fn build_app(state: AppState) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .web_origin
                .parse::<HeaderValue>()
                .expect("invalid WEB_ORIGIN"),
        )
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any())
        .allow_credentials(true);

    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1", routes::api_routes())
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .layer(cors)
        .with_state(state)
}
