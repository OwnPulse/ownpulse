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
pub mod config;
pub mod dashboard;
pub mod events;
pub mod explore;
pub mod export;
pub mod friends;
pub mod garmin;
pub mod genetics;
pub mod health_records;
pub mod healthkit;
pub mod insights;
pub mod integrations;
pub mod interventions;
pub mod labs;
pub mod observations;
pub mod observer_polls;
pub mod oura;
pub mod protocols;
pub mod saved_medicines;
pub mod sleep;
pub mod source_preferences;
pub mod stats;
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

/// User-initiated auth routes that need spam protection via rate limiting.
fn rate_limited_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/google/login", get(auth::google_login))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .route("/auth/garmin/login", get(garmin::garmin_login))
        .route("/auth/oura/login", get(oura::oura_login))
}

/// OAuth callback routes that are server-initiated redirects protected by
/// CSRF cookies / state parameters. These do NOT need rate limiting — applying
/// the auth rate limit here causes legitimate OAuth flows to lock users out
/// because login + callback together consume two tokens.
fn oauth_callback_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/google/callback", get(auth::google_callback))
        .route("/auth/apple/callback", post(auth::apple_callback))
        .route("/auth/garmin/callback", get(garmin::garmin_callback))
        .route("/auth/oura/callback", get(oura::oura_callback))
}

/// Build the versioned API router with rate limiting on auth, explore, and
/// observer-poll endpoints. Mounted under `/api/v1` by `build_app`.
pub fn api_routes() -> Router<AppState> {
    use tower_governor::{
        GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
    };

    // --- Auth: 10 req/min per IP ---
    // SmartIpKeyExtractor checks X-Forwarded-For/X-Real-IP headers first,
    // falling back to peer address. Required when behind a reverse proxy.
    // OAuth callbacks are excluded — they are server-initiated redirects
    // protected by CSRF state, not user-initiated spam vectors.
    let auth_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(6) // replenish 1 token every 6s -> 10/min
        .burst_size(10)
        .finish()
        .expect("failed to build governor config");

    let rate_limited_auth = rate_limited_auth_routes().layer(GovernorLayer {
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

    // --- Invite check: 10 req/min per IP ---
    let invite_check_governor_conf = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(6) // replenish 1 token every 6s -> 10/min
        .burst_size(10)
        .finish()
        .expect("failed to build invite check governor config");

    let rate_limited_invite_check = invite_check_routes().layer(GovernorLayer {
        config: invite_check_governor_conf.into(),
    });

    base_routes()
        .merge(rate_limited_auth)
        .merge(oauth_callback_routes())
        .merge(rate_limited_explore)
        .merge(rate_limited_poll_accept)
        .merge(rate_limited_poll_respond)
        .merge(rate_limited_invite_check)
}

/// Build the versioned API router without rate limiting.
/// Used by integration tests where `ConnectInfo` is not available.
pub fn api_routes_without_rate_limit() -> Router<AppState> {
    base_routes()
        .merge(rate_limited_auth_routes())
        .merge(oauth_callback_routes())
        .merge(explore_routes())
        .merge(poll_accept_routes())
        .merge(poll_respond_routes())
        .merge(invite_check_routes())
}

fn explore_routes() -> Router<AppState> {
    Router::new()
        .route("/explore/interventions", get(explore::interventions))
        .route("/explore/metrics", get(explore::metrics))
        .route("/explore/series", get(explore::series_get))
        .route("/explore/series", post(explore::series_post))
        .route("/explore/batch-series", post(explore::batch_series))
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

fn invite_check_routes() -> Router<AppState> {
    Router::new().route("/invites/:code/check", get(admin::check_invite))
}

fn base_routes() -> Router<AppState> {
    Router::new()
        // Config (unauthenticated)
        .route("/config", get(config::get_config))
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
        // Saved medicines
        .route("/saved-medicines", get(saved_medicines::list))
        .route("/saved-medicines", post(saved_medicines::create))
        .route("/saved-medicines/:id", put(saved_medicines::update))
        .route("/saved-medicines/:id", delete(saved_medicines::delete))
        // Check-ins
        .route("/checkins", post(checkins::create))
        .route("/checkins", get(checkins::list))
        .route("/checkins/:id", get(checkins::get))
        .route("/checkins/:id", put(checkins::update))
        .route("/checkins/:id", delete(checkins::delete))
        // Observations
        .route("/observations", post(observations::create))
        .route("/observations", get(observations::list))
        .route("/observations/:id", get(observations::get))
        .route("/observations/:id", delete(observations::delete))
        // Lab results
        .route("/labs", post(labs::create))
        .route("/labs", get(labs::list))
        .route("/labs/bulk", post(labs::bulk_create))
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
        // Genetics
        .route("/genetics/upload", post(genetics::upload))
        .route("/genetics/summary", get(genetics::summary))
        .route("/genetics/interpretations", get(genetics::interpretations))
        .route("/genetics", get(genetics::list))
        .route("/genetics", delete(genetics::delete_all))
        // Insights
        .route("/insights", get(insights::list))
        .route("/insights/generate", post(insights::generate))
        .route("/insights/:id/dismiss", post(insights::dismiss))
        // Dashboard
        .route("/dashboard/summary", get(dashboard::summary))
        // Admin
        .route("/admin/users", get(admin::list_users))
        .route("/admin/users/:id/role", patch(admin::update_role))
        .route("/admin/users/:id/status", patch(admin::update_status))
        .route("/admin/users/:id", delete(admin::delete_user))
        .route("/admin/invites", post(admin::create_invite))
        .route("/admin/invites", get(admin::list_invites))
        .route("/admin/invites/stats", get(admin::invite_stats))
        .route("/admin/invites/:id/claims", get(admin::invite_claims))
        .route(
            "/admin/invites/:id/send-email",
            post(admin::send_invite_email),
        )
        .route("/admin/invites/:id", delete(admin::revoke_invite))
        // Admin feature flags
        .route("/admin/feature-flags", get(admin::list_feature_flags))
        .route("/admin/feature-flags/:key", put(admin::upsert_feature_flag))
        .route(
            "/admin/feature-flags/:key",
            delete(admin::delete_feature_flag),
        )
        // Admin protocol endpoints
        .route("/admin/protocols/import", post(admin::admin_bulk_import))
        .route(
            "/admin/protocols/:id/promote",
            post(admin::promote_protocol),
        )
        .route("/admin/protocols/:id/demote", post(admin::demote_protocol))
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
        // Protocols — shared/import/template routes before :id to avoid UUID matching
        .route(
            "/protocols/shared/:token",
            get(protocols::get_shared_protocol),
        )
        .route("/protocols/import/:token", post(protocols::import_protocol))
        .route("/protocols/import", post(protocols::import_protocol_file))
        .route("/protocols/templates", get(protocols::list_templates))
        .route(
            "/protocols/templates/:id/copy",
            post(protocols::copy_template),
        )
        .route("/protocols/runs/todays-doses", get(protocols::todays_doses))
        .route(
            "/protocols/active-substances",
            get(protocols::active_substances),
        )
        .route("/protocols/runs/active", get(protocols::list_active_runs))
        .route("/protocols/runs/:run_id", patch(protocols::update_run))
        .route(
            "/protocols/runs/:run_id/doses/log",
            post(protocols::log_dose_on_run),
        )
        .route(
            "/protocols/runs/:run_id/doses/skip",
            post(protocols::skip_dose_on_run),
        )
        .route(
            "/protocols/notifications",
            get(protocols::get_notification_preferences),
        )
        .route(
            "/protocols/notifications",
            put(protocols::update_notification_preferences),
        )
        .route(
            "/notifications/push-token",
            post(protocols::register_push_token),
        )
        .route(
            "/notifications/push-token/:device_token",
            delete(protocols::delete_push_token),
        )
        .route("/protocols", post(protocols::create_protocol))
        .route("/protocols", get(protocols::list_protocols))
        .route("/protocols/:id", get(protocols::get_protocol))
        .route("/protocols/:id", patch(protocols::update_protocol))
        .route("/protocols/:id", delete(protocols::delete_protocol))
        .route("/protocols/:id/export", get(protocols::export_protocol))
        .route("/protocols/:id/runs", post(protocols::create_run))
        .route("/protocols/:id/runs", get(protocols::list_runs))
        .route("/protocols/:id/doses/log", post(protocols::log_dose))
        .route("/protocols/:id/doses/skip", post(protocols::skip_dose))
        .route("/protocols/:id/share", post(protocols::share_protocol))
        // Stats — correlation explorer
        .route("/stats/before-after", post(stats::before_after))
        .route("/stats/correlate", post(stats::correlate))
        .route("/stats/lag-correlate", post(stats::lag_correlate))
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
