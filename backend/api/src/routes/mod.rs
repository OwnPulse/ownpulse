// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Route handlers.
//!
//! Each sub-module defines handlers for one route group.

pub mod account;
pub mod admin;
pub mod audit;
pub mod auth;
pub mod checkins;
pub mod dashboard;
pub mod events;
pub mod explore;
pub mod export;
pub mod friends;
pub mod health_records;
pub mod healthkit;
pub mod integrations;
pub mod interventions;
pub mod labs;
pub mod observations;
pub mod observer_polls;
pub mod sleep;
pub mod source_preferences;
pub mod stats;
pub mod waitlist;

use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};

use crate::AppState;

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/google/login", get(auth::google_login))
        .route("/auth/google/callback", get(auth::google_callback))
        .route("/auth/apple/callback", post(auth::apple_callback))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
}

/// Build the versioned API router with rate limiting on auth endpoints.
/// Mounted under `/api/v1` by `build_app`.
pub fn api_routes() -> Router<AppState> {
    use tower_governor::{
        GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
    };

    // 5 requests per 60 seconds per IP on auth endpoints.
    // SmartIpKeyExtractor checks X-Forwarded-For/X-Real-IP headers first,
    // falling back to peer address. Required when behind a reverse proxy.
    let auth_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(12) // replenish 1 token every 12s → 5/min
        .burst_size(5)
        .finish()
        .expect("failed to build governor config");

    let rate_limited_auth = auth_routes().layer(GovernorLayer {
        config: auth_governor_conf.into(),
    });

    base_routes().merge(rate_limited_auth)
}

/// Build the versioned API router without rate limiting.
/// Used by integration tests where `ConnectInfo` is not available.
pub fn api_routes_without_rate_limit() -> Router<AppState> {
    base_routes().merge(auth_routes())
}

fn base_routes() -> Router<AppState> {
    Router::new()
        // Waitlist (unauthenticated)
        .route("/waitlist", post(waitlist::signup))
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
        .route("/account/audit-log", get(audit::list_audit_log))
        // Export
        .route("/export/json", get(export::export_json))
        .route("/export/csv", get(export::export_csv))
        // Sleep records (stored as observations with type='sleep')
        .route("/sleep", post(sleep::create))
        .route("/sleep", get(sleep::list))
        .route("/sleep/:id", get(sleep::get))
        .route("/sleep/:id", delete(sleep::delete))
        // Integrations
        .route("/integrations", get(integrations::list))
        .route("/integrations/:source", delete(integrations::disconnect))
        // Dashboard
        .route("/dashboard/summary", get(dashboard::summary))
        // Admin
        .route("/admin/users", get(admin::list_users))
        .route("/admin/users/:id/role", patch(admin::update_role))
        .route("/admin/users/:id/status", patch(admin::update_status))
        .route("/admin/users/:id", delete(admin::delete_user))
        .route("/admin/invites", post(admin::create_invite))
        .route("/admin/invites", get(admin::list_invites))
        .route("/admin/invites/:id", delete(admin::revoke_invite))
        // Auth methods (authenticated)
        .route("/auth/methods", get(auth::list_auth_methods))
        .route("/auth/link", post(auth::link_auth))
        .route("/auth/link/:provider", delete(auth::unlink_auth))
        // Friend sharing
        .route("/friends/shares", post(friends::create_share))
        .route("/friends/shares/outgoing", get(friends::list_outgoing))
        .route("/friends/shares/incoming", get(friends::list_incoming))
        .route("/friends/shares/accept-link", post(friends::accept_link))
        .route("/friends/shares/:id/accept", post(friends::accept_share))
        .route("/friends/shares/:id", delete(friends::revoke_share))
        .route(
            "/friends/shares/:id/permissions",
            patch(friends::update_permissions),
        )
        .route("/friends/:friend_id/data", get(friends::get_friend_data))
        // Stats — correlation explorer
        .route("/stats/before-after", post(stats::before_after))
        .route("/stats/correlate", post(stats::correlate))
        .route("/stats/lag-correlate", post(stats::lag_correlate))
        // Explore — metrics, time-series, saved charts, intervention markers
        .route("/explore/interventions", get(explore::interventions))
        .route("/explore/metrics", get(explore::metrics))
        .route("/explore/series", get(explore::series_get))
        .route("/explore/series", post(explore::series_post))
        .route("/explore/charts", post(explore::create_chart))
        .route("/explore/charts", get(explore::list_charts))
        .route("/explore/charts/:id", get(explore::get_chart))
        .route(
            "/explore/charts/:id",
            axum::routing::put(explore::update_chart),
        )
        .route("/explore/charts/:id", delete(explore::delete_chart))
        // SSE events (auth via query param, not middleware)
        .route("/events", get(events::events_stream))
        // Observer polls — owner endpoints
        .route("/observer-polls", post(observer_polls::create_poll))
        .route("/observer-polls", get(observer_polls::list_polls))
        // Observer polls — observer endpoints (must be before :id routes)
        .route(
            "/observer-polls/accept",
            post(observer_polls::accept_invite),
        )
        .route("/observer-polls/my-polls", get(observer_polls::my_polls))
        .route(
            "/observer-polls/export",
            get(observer_polls::export_responses),
        )
        .route(
            "/observer-polls/responses/:response_id",
            delete(observer_polls::delete_response),
        )
        // Observer polls — :id routes
        .route("/observer-polls/:id", get(observer_polls::get_poll))
        .route("/observer-polls/:id", patch(observer_polls::update_poll))
        .route("/observer-polls/:id", delete(observer_polls::delete_poll))
        .route(
            "/observer-polls/:id/invite",
            post(observer_polls::create_invite),
        )
        .route(
            "/observer-polls/:id/responses",
            get(observer_polls::list_responses),
        )
        .route(
            "/observer-polls/:id/respond",
            put(observer_polls::submit_response),
        )
        .route(
            "/observer-polls/:id/my-responses",
            get(observer_polls::my_responses),
        )
}
