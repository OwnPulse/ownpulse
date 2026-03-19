// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Route handlers.
//!
//! Each sub-module defines handlers for one route group.

pub mod account;
pub mod auth;
pub mod checkins;
pub mod export;
pub mod health_records;
pub mod healthkit;
pub mod integrations;
pub mod interventions;
pub mod labs;
pub mod observations;
pub mod source_preferences;
pub mod waitlist;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::AppState;

/// Build the versioned API router. Mounted under `/api/v1` by `build_app`.
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // Waitlist (unauthenticated)
        .route("/waitlist", post(waitlist::signup))
        // Auth (unauthenticated)
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/google/callback", get(auth::google_callback))
        // Health records
        .route("/health-records", post(health_records::create).get(health_records::list))
        .route(
            "/health-records/:id",
            get(health_records::get).delete(health_records::delete),
        )
        // Interventions
        .route(
            "/interventions",
            post(interventions::create).get(interventions::list),
        )
        .route(
            "/interventions/:id",
            get(interventions::get).delete(interventions::delete),
        )
        // Checkins
        .route("/checkins", post(checkins::upsert).get(checkins::list))
        .route(
            "/checkins/:id",
            get(checkins::get).delete(checkins::delete),
        )
        // Observations
        .route(
            "/observations",
            post(observations::create).get(observations::list),
        )
        .route(
            "/observations/:id",
            get(observations::get).delete(observations::delete),
        )
        // Labs
        .route("/labs", post(labs::create).get(labs::list))
        .route("/labs/:id", get(labs::get).delete(labs::delete))
        // HealthKit
        .route("/healthkit/sync", post(healthkit::bulk_insert))
        .route("/healthkit/write-queue", get(healthkit::write_queue))
        .route("/healthkit/confirm", post(healthkit::confirm))
        // Source preferences
        .route(
            "/source-preferences",
            get(source_preferences::list).post(source_preferences::upsert),
        )
        // Account
        .route("/account", get(account::get_account).delete(account::delete_account))
        // Export
        .route("/export/json", get(export::export_json))
        .route("/export/csv", get(export::export_csv))
        // Integrations
        .route("/integrations", get(integrations::list))
        .route("/integrations/:source", delete(integrations::disconnect))
}
