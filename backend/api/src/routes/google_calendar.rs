// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google Calendar OAuth 2.0 connect flow — login, callback, and manual sync
//! routes. Separate from `/auth/google/login` (account login/signup): this
//! flow requires an already-authenticated user, requests the
//! `calendar.readonly` scope, and stores its token under
//! `integration_tokens.source = 'google_calendar'`.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::crypto;
use crate::db::integration_tokens;
use crate::error::ApiError;
use crate::integrations::google;
use crate::jobs::google_calendar_sync;
use crate::routes::read_cookie;

/// GET /auth/google-calendar/login — start the OAuth 2.0 connect flow.
///
/// Requires authentication. Generates a CSRF state parameter, stores it in a
/// short-lived httpOnly cookie, and redirects to Google's authorization page
/// requesting the read-only Calendar scope.
pub async fn google_calendar_login(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not configured".to_string()))?;

    let redirect_uri = state.config.google_calendar_redirect_uri();
    let csrf_state = Uuid::new_v4().to_string();

    // `access_type=offline` + `prompt=consent` are required to receive a
    // refresh_token — without them Google only returns one on the very first
    // authorization for this scope, which would silently break re-connects.
    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &access_type=offline\
         &prompt=consent\
         &state={}",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode("https://www.googleapis.com/auth/calendar.readonly"),
        urlencoding::encode(&csrf_state),
    );

    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };

    let state_cookie = format!(
        "google_calendar_oauth_state={csrf_state}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600"
    );

    let mut response = Redirect::to(&auth_url).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        state_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );

    tracing::debug!(user_id = %auth_user.id, "Google Calendar connect flow started");

    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleCalendarCallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /auth/google-calendar/callback — exchange the authorization code for
/// tokens.
///
/// Requires authentication. Validates the CSRF state against the cookie,
/// exchanges the code for tokens, encrypts and stores them under
/// `source = 'google_calendar'`.
pub async fn google_calendar_callback(
    State(state): State<AppState>,
    auth_user: AuthUser,
    headers: axum::http::HeaderMap,
    Query(query): Query<GoogleCalendarCallbackQuery>,
) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not configured".to_string()))?;
    let client_secret = state
        .config
        .google_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_SECRET not configured".to_string()))?;

    let expected_state = read_cookie(&headers, "google_calendar_oauth_state")
        .ok_or_else(|| ApiError::BadRequest("missing google_calendar_oauth_state cookie".into()))?;

    if expected_state != query.state {
        return Err(ApiError::BadRequest("OAuth state mismatch".into()));
    }

    let redirect_uri = state.config.google_calendar_redirect_uri();

    let tokens = google::exchange_code_for_tokens(
        &state.http_client,
        client_id,
        client_secret,
        &redirect_uri,
        &query.code,
        &state.config.google_token_url,
        None,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("Google Calendar token exchange failed: {e}")))?;

    let encryption_key = crypto::parse_encryption_key(&state.config.encryption_key)?;

    let expires_at = tokens
        .expires_in
        .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

    // Google only reliably reissues a refresh_token on a user's *first*
    // authorization for this scope (`prompt=consent` in `google_calendar_login`
    // makes a subsequent one likely too, but isn't guaranteed for every
    // provider edge case) — if this exchange didn't return one, keep
    // whatever was already stored from a prior connect rather than nulling
    // it out and silently breaking the background sync job's ability to
    // refresh later.
    let prev_key = state
        .config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()?;

    let existing_refresh_token = integration_tokens::list_for_user(
        &state.pool,
        auth_user.id,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    .map_err(|e| {
        ApiError::Internal(format!(
            "failed to load existing Google Calendar token: {e}"
        ))
    })?
    .into_iter()
    .find(|t| t.source == "google_calendar")
    .and_then(|t| t.refresh_token);

    let refresh_to_store = tokens
        .refresh_token
        .as_deref()
        .or(existing_refresh_token.as_deref());

    integration_tokens::upsert(
        &state.pool,
        auth_user.id,
        "google_calendar",
        &tokens.access_token,
        refresh_to_store,
        expires_at,
        &encryption_key,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("failed to store Google Calendar tokens: {e}")))?;

    tracing::info!(user_id = %auth_user.id, "Google Calendar integration connected");

    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let clear_state = format!(
        "google_calendar_oauth_state=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0"
    );

    let redirect_url = format!(
        "{}/settings?connected=google_calendar",
        state.config.web_origin
    );
    let mut response = Redirect::to(&redirect_url).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        clear_state
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );

    Ok(response)
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub source: String,
    pub records_inserted: u32,
}

/// POST /integrations/google-calendar/sync — fetch calendar aggregates now
/// instead of waiting for the periodic background job's next interval.
pub async fn sync(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<SyncResponse>, ApiError> {
    let records_inserted = google_calendar_sync::sync_user_now(
        &state.pool,
        &state.config,
        &state.http_client,
        auth_user.id,
        &state.event_tx,
    )
    .await
    .map_err(|e| {
        if let crate::jobs::SyncError::Upstream(ref msg) = e {
            tracing::warn!(user_id = %auth_user.id, error = %msg, "Google Calendar manual sync failed");
        }
        ApiError::from(e)
    })?;

    Ok(Json(SyncResponse {
        source: "google_calendar".to_string(),
        records_inserted,
    }))
}
