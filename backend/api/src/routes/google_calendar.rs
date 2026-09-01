// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google Calendar OAuth 2.0 connect flow — login, callback, and manual sync
//! routes. Separate from `/auth/google/login` (account login/signup): this
//! flow requires an already-authenticated user, requests the
//! `calendar.readonly` scope, and stores its token under
//! `integration_tokens.source = 'google_calendar'`.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::crypto;
use crate::db::{integration_tokens, oauth_states};
use crate::error::ApiError;
use crate::integrations::google;
use crate::jobs::google_calendar_sync;

const PROVIDER: &str = "google_calendar";

#[derive(Serialize)]
pub struct GoogleCalendarLoginResponse {
    pub auth_url: String,
}

/// GET /auth/google-calendar/login — start the OAuth 2.0 connect flow.
///
/// A JSON endpoint, not a redirect: the web app is same-origin with the API
/// (the default `google_calendar_redirect_uri` is `{WEB_ORIGIN}/...`), so it
/// calls this like any other authenticated endpoint (Bearer header) and
/// navigates the browser to the returned `auth_url` itself.
///
/// Records which user started the flow in `oauth_states`, keyed by the CSRF
/// `state` value handed to Google. `google_calendar_callback` — a full-page
/// GET the browser follows when Google redirects back, which cannot carry
/// an `Authorization` header — looks the row up to both validate CSRF and
/// recover identity, instead of relying on a cookie to carry either.
pub async fn google_calendar_login(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<GoogleCalendarLoginResponse>, ApiError> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not configured".to_string()))?;

    let redirect_uri = state.config.google_calendar_redirect_uri();
    let csrf_state = Uuid::new_v4();

    oauth_states::insert(&state.pool, csrf_state, auth_user.id, PROVIDER)
        .await
        .map_err(|e| ApiError::Internal(format!("failed to store OAuth state: {e}")))?;

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
         &state={csrf_state}",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode("https://www.googleapis.com/auth/calendar.readonly"),
    );

    tracing::debug!(user_id = %auth_user.id, "Google Calendar connect flow started");

    Ok(Json(GoogleCalendarLoginResponse { auth_url }))
}

#[derive(Deserialize)]
pub struct GoogleCalendarCallbackQuery {
    /// Absent when Google reports `error` instead of granting consent.
    pub code: Option<String>,
    pub state: String,
    /// Set by Google on the deny path (e.g. `access_denied`) instead of `code`.
    pub error: Option<String>,
}

/// Build a redirect back to the web Sources page carrying a short,
/// non-sensitive error code for it to render — never raw JSON or an error
/// page, since every path through this handler is a browser navigation.
fn error_redirect(web_origin: &str, code: &str) -> Response {
    let url = format!("{web_origin}/sources?error={}", urlencoding::encode(code));
    Redirect::to(&url).into_response()
}

/// GET /auth/google-calendar/callback — exchange the authorization code for
/// tokens.
///
/// This is a full-page GET the browser follows after Google redirects back,
/// so — like `google_calendar_login` — it cannot carry an `Authorization`
/// header. It authenticates by consuming the `oauth_states` row matching
/// `state` (single-use: deleted on read, and rejected if older than
/// [`oauth_states::STATE_TTL_MINUTES`]), which both validates CSRF and
/// recovers the user id that started the flow — no cookie involved.
/// Exchanges the code for tokens and stores them encrypted under
/// `source = 'google_calendar'`. Every path — success or failure — ends in
/// a redirect to the web app; nothing here is ever rendered as JSON to a
/// browser.
pub async fn google_calendar_callback(
    State(state): State<AppState>,
    Query(query): Query<GoogleCalendarCallbackQuery>,
) -> Response {
    let web_origin = &state.config.web_origin;

    let Ok(state_uuid) = Uuid::parse_str(&query.state) else {
        return error_redirect(web_origin, "state_invalid");
    };

    let user_id = match oauth_states::consume(&state.pool, state_uuid, PROVIDER).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => return error_redirect(web_origin, "state_invalid"),
        Err(e) => {
            tracing::error!(error = %e, "failed to consume Google Calendar OAuth state");
            return error_redirect(web_origin, "server_error");
        }
    };

    // The state row is already consumed above regardless of outcome, so a
    // retried callback (deny or otherwise) can't reuse it.
    if let Some(provider_error) = query.error {
        tracing::info!(user_id = %user_id, error = %provider_error, "Google Calendar connect flow declined or failed at provider");
        return error_redirect(web_origin, "access_denied");
    }

    let Some(code) = query.code else {
        return error_redirect(web_origin, "missing_code");
    };

    let Some(client_id) = state.config.google_client_id.as_deref() else {
        tracing::error!("Google Calendar callback reached with GOOGLE_CLIENT_ID unconfigured");
        return error_redirect(web_origin, "server_error");
    };
    let Some(client_secret) = state.config.google_client_secret.as_deref() else {
        tracing::error!("Google Calendar callback reached with GOOGLE_CLIENT_SECRET unconfigured");
        return error_redirect(web_origin, "server_error");
    };

    let redirect_uri = state.config.google_calendar_redirect_uri();

    let tokens = match google::exchange_code_for_tokens(
        &state.http_client,
        client_id,
        client_secret,
        &redirect_uri,
        &code,
        &state.config.google_token_url,
        None,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(user_id = %user_id, error = %e, "Google Calendar token exchange failed");
            return error_redirect(web_origin, "exchange_failed");
        }
    };

    let encryption_key = match crypto::parse_encryption_key(&state.config.encryption_key) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = %e, "bad ENCRYPTION_KEY configured");
            return error_redirect(web_origin, "server_error");
        }
    };

    let expires_at = tokens
        .expires_in
        .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

    let prev_key = match state
        .config
        .encryption_key_previous
        .as_ref()
        .map(|k| crypto::parse_encryption_key(k))
        .transpose()
    {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = %e, "bad ENCRYPTION_KEY_PREVIOUS configured");
            return error_redirect(web_origin, "server_error");
        }
    };

    // Google only reliably reissues a refresh_token on a user's *first*
    // authorization for this scope (`prompt=consent` in `google_calendar_login`
    // makes a subsequent one likely too, but isn't guaranteed for every
    // provider edge case) — if this exchange didn't return one, keep
    // whatever was already stored from a prior connect rather than nulling
    // it out and silently breaking the background sync job's ability to
    // refresh later.
    let existing_refresh_token = match integration_tokens::list_for_user(
        &state.pool,
        user_id,
        &encryption_key,
        prev_key.as_ref(),
    )
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .find(|t| t.source == PROVIDER)
            .and_then(|t| t.refresh_token),
        Err(e) => {
            tracing::error!(error = %e, user_id = %user_id, "failed to load existing Google Calendar token");
            return error_redirect(web_origin, "server_error");
        }
    };

    let refresh_to_store = tokens
        .refresh_token
        .as_deref()
        .or(existing_refresh_token.as_deref());

    if let Err(e) = integration_tokens::upsert(
        &state.pool,
        user_id,
        PROVIDER,
        &tokens.access_token,
        refresh_to_store,
        expires_at,
        &encryption_key,
    )
    .await
    {
        tracing::error!(error = %e, user_id = %user_id, "failed to store Google Calendar tokens");
        return error_redirect(web_origin, "server_error");
    }

    tracing::info!(user_id = %user_id, "Google Calendar integration connected");

    Redirect::to(&format!("{web_origin}/sources?connected=google_calendar")).into_response()
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
