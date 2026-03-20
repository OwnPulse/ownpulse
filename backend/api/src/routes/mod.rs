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
pub mod sleep;
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
        // Auth
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/google/callback", get(auth::google_callback))
        // Health records
        .route("/health-records", post(health_records::create))
        .route("/health-records", get(health_records::list))
        .route("/health-records/:id", get(health_records::get))
        .route("/health-records/:id", delete(health_records::delete))
        // Interventions
        .route("/interventions", post(interventions::create))
        .route("/interventions", get(interventions::list))
        .route("/interventions/:id", get(interventions::get))
        .route("/interventions/:id", delete(interventions::delete))
        // Check-ins (POST is upsert)
        .route("/checkins", post(checkins::upsert))
        .route("/checkins", get(checkins::list))
        .route("/checkins/:id", get(checkins::get))
        .route("/checkins/:id", delete(checkins::delete))
        // Observations
        .route("/observations", post(observations::create))
        .route("/observations", get(observations::list))
        .route("/observations/:id", get(observations::get))
        .route("/observations/:id", delete(observations::delete))
        // Lab results
        .route("/labs", post(labs::create))
        .route("/labs", get(labs::list))
        .route("/labs/:id", get(labs::get))
        .route("/labs/:id", delete(labs::delete))
        // HealthKit sync
        .route("/healthkit/sync", post(healthkit::bulk_insert))
        .route("/healthkit/write-queue", get(healthkit::write_queue))
        .route("/healthkit/confirm", post(healthkit::confirm))
        // Source preferences
        .route("/source-preferences", get(source_preferences::list))
        .route("/source-preferences", post(source_preferences::upsert))
        // Account
        .route("/account", get(account::get_account))
        .route("/account", delete(account::delete_account))
        // Export
        .route("/export/json", get(export::export_json))
        .route("/export/csv", get(export::export_csv))
        // Sleep records
        .route("/sleep", post(sleep::create))
        .route("/sleep", get(sleep::list))
        .route("/sleep/:id", get(sleep::get))
        .route("/sleep/:id", delete(sleep::delete))
        // Integrations
        .route("/integrations", get(integrations::list))
        .route("/integrations/:source", delete(integrations::disconnect))
}
