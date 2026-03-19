// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Route handlers.
//!
//! Each sub-module defines handlers for one route group.

pub mod auth;
pub mod waitlist;

use axum::{
    routing::{get, post},
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
}
