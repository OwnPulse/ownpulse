// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

mod auth;
pub mod config;
mod crypto;
mod db;
mod export;
mod integrations;
mod jobs;
mod models;
pub mod routes;
mod stats;

use axum::{routing::{get, post}, Json, Router};
use axum_prometheus::PrometheusMetricLayer;
use serde_json::json;
use sqlx::PgPool;

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

/// Build the application router. Extracted so integration tests can reuse it.
pub fn build_app(pool: PgPool) -> Router {
    let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/waitlist", post(routes::waitlist::signup))
        .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(prometheus_layer)
        .with_state(pool)
}
