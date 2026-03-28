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
pub mod waitlist;

use axum::http;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use tower_governor::errors::GovernorError;
use tower_governor::key_extractor::KeyExtractor;

use crate::AppState;

/// A [`KeyExtractor`] that extracts the user ID from the JWT `sub` claim
/// in the `Authorization: Bearer <token>` header.
///
/// This performs a *lightweight* base64 decode of the JWT payload to read the
/// `sub` field — it does **not** verify the signature. Full verification happens
/// later in the [`AuthUser`](crate::auth::extractor::AuthUser) extractor. The
/// rate limiter only needs a consistent key; an invalid/expired token will be
/// rejected by the handler before any real work is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JwtSubjectKeyExtractor;

impl KeyExtractor for JwtSubjectKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &http::Request<T>) -> Result<Self::Key, GovernorError> {
        let header = req
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|v: &http::HeaderValue| v.to_str().ok())
            .and_then(|v: &str| v.strip_prefix("Bearer "))
            .ok_or(GovernorError::UnableToExtractKey)?;

        // JWT is three base64url segments separated by dots.
        let payload_b64 = header
            .split('.')
            .nth(1)
            .ok_or(GovernorError::UnableToExtractKey)?;

        // base64url decode (JWT uses URL-safe alphabet, no padding).
        let payload_bytes =
            base64url_decode(payload_b64).map_err(|_| GovernorError::UnableToExtractKey)?;

        // Extract the `sub` field with minimal parsing.
        #[derive(serde::Deserialize)]
        struct Sub {
            sub: String,
        }

        let parsed: Sub = serde_json::from_slice(&payload_bytes)
            .map_err(|_| GovernorError::UnableToExtractKey)?;

        Ok(parsed.sub)
    }
}

/// Decode base64url (RFC 4648 section 5) without padding, as used in JWTs.
fn base64url_decode(input: &str) -> Result<Vec<u8>, ()> {
    // Replace URL-safe chars with standard base64 chars and add padding.
    let mut s = input.replace('-', "+").replace('_', "/");
    match s.len() % 4 {
        2 => s.push_str("=="),
        3 => s.push('='),
        0 => {}
        _ => return Err(()),
    }
    // Use a minimal inline base64 decoder via the data_encoding-style approach.
    // Since we already have the `hex` crate but not `base64` in prod deps,
    // we can use a small manual decoder or leverage serde_json indirectly.
    // Actually, we can use the engine from jsonwebtoken's dependency.
    // Simplest: manual decode using a lookup table.
    decode_base64_standard(&s)
}

fn decode_base64_standard(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=')
        .map(|b| {
            TABLE
                .iter()
                .position(|&c| c == b)
                .map(|p| p as u8)
                .ok_or(())
        })
        .collect::<Result<_, _>>()?;

    for chunk in bytes.chunks(4) {
        match chunk.len() {
            4 => {
                out.push((chunk[0] << 2) | (chunk[1] >> 4));
                out.push((chunk[1] << 4) | (chunk[2] >> 2));
                out.push((chunk[2] << 6) | chunk[3]);
            }
            3 => {
                out.push((chunk[0] << 2) | (chunk[1] >> 4));
                out.push((chunk[1] << 4) | (chunk[2] >> 2));
            }
            2 => {
                out.push((chunk[0] << 2) | (chunk[1] >> 4));
            }
            _ => return Err(()),
        }
    }

    Ok(out)
}

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

/// Build the versioned API router with rate limiting on auth, explore, and
/// observer-poll endpoints. Mounted under `/api/v1` by `build_app`.
pub fn api_routes() -> Router<AppState> {
    use tower_governor::{
        GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
    };

    // --- Auth: 5 req/min per IP ---
    // SmartIpKeyExtractor checks X-Forwarded-For/X-Real-IP headers first,
    // falling back to peer address. Required when behind a reverse proxy.
    let auth_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(12) // replenish 1 token every 12s -> 5/min
        .burst_size(5)
        .finish()
        .expect("failed to build governor config");

    let rate_limited_auth = auth_routes().layer(GovernorLayer {
        config: auth_governor_conf.into(),
    });

    // --- Explore: 30 req/min per user (JWT sub) ---
    let explore_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(JwtSubjectKeyExtractor)
        .per_second(2) // replenish 1 token every 2s -> 30/min
        .burst_size(30)
        .finish()
        .expect("failed to build explore governor config");

    let rate_limited_explore = explore_routes().layer(GovernorLayer {
        config: explore_governor_conf.into(),
    });

    // --- Observer poll accept: 10 req/min per IP ---
    let poll_accept_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(6) // replenish 1 token every 6s -> 10/min
        .burst_size(10)
        .finish()
        .expect("failed to build poll accept governor config");

    let rate_limited_poll_accept = poll_accept_routes().layer(GovernorLayer {
        config: poll_accept_governor_conf.into(),
    });

    // --- Observer poll respond: 10 req/min per user (JWT sub) ---
    let poll_respond_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(JwtSubjectKeyExtractor)
        .per_second(6) // replenish 1 token every 6s -> 10/min
        .burst_size(10)
        .finish()
        .expect("failed to build poll respond governor config");

    let rate_limited_poll_respond = poll_respond_routes().layer(GovernorLayer {
        config: poll_respond_governor_conf.into(),
    });

    base_routes()
        .merge(rate_limited_auth)
        .merge(rate_limited_explore)
        .merge(rate_limited_poll_accept)
        .merge(rate_limited_poll_respond)
}

/// Build the versioned API router without rate limiting.
/// Used by integration tests where `ConnectInfo` is not available.
pub fn api_routes_without_rate_limit() -> Router<AppState> {
    base_routes()
        .merge(auth_routes())
        .merge(explore_routes())
        .merge(poll_accept_routes())
        .merge(poll_respond_routes())
}

fn explore_routes() -> Router<AppState> {
    Router::new()
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
}

fn poll_accept_routes() -> Router<AppState> {
    Router::new().route(
        "/observer-polls/accept",
        post(observer_polls::accept_invite),
    )
}

fn poll_respond_routes() -> Router<AppState> {
    Router::new().route(
        "/observer-polls/:id/respond",
        put(observer_polls::submit_response),
    )
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
        // Explore routes are in explore_routes() for rate limiting
        // SSE events (auth via query param, not middleware)
        .route("/events", get(events::events_stream))
        // Observer polls — owner endpoints
        .route("/observer-polls", post(observer_polls::create_poll))
        .route("/observer-polls", get(observer_polls::list_polls))
        // Observer polls — observer endpoints (must be before :id routes)
        // observer-polls/accept is in poll_accept_routes() for rate limiting
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
        // observer-polls/:id/respond is in poll_respond_routes() for rate limiting
        .route(
            "/observer-polls/:id/my-responses",
            get(observer_polls::my_responses),
        )
}
