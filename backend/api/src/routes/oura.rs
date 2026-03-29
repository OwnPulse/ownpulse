// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Oura OAuth 2.0 flow — connect and callback routes.

use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::crypto;
use crate::db::integration_tokens;
use crate::error::ApiError;
use crate::integrations::oura::OuraClient;

/// GET /auth/oura/login — start the OAuth 2.0 flow.
///
/// Requires authentication. Generates a CSRF state parameter, stores it in a
/// short-lived httpOnly cookie, and redirects to Oura's authorization page.
pub async fn oura_login(
    State(state): State<AppState>,
    AuthUser { .. }: AuthUser,
) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .oura_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("OURA_CLIENT_ID not configured".to_string()))?;

    let redirect_uri = format!("{}/api/v1/auth/oura/callback", state.config.web_origin);
    let csrf_state = Uuid::new_v4().to_string();

    let client = OuraClient::new(
        client_id.to_string(),
        String::new(),
        state.config.oura_api_base_url.clone(),
        state.config.oura_auth_base_url.clone(),
        state.http_client.clone(),
    );

    let auth_url = client.authorization_url(&redirect_uri, &csrf_state);

    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };

    let state_cookie = format!(
        "oura_oauth_state={csrf_state}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600"
    );

    let mut response = Redirect::to(&auth_url).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        state_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );

    Ok(response)
}

#[derive(Deserialize)]
pub struct OuraCallbackQuery {
    pub code: String,
    pub state: String,
}

/// GET /auth/oura/callback — exchange the authorization code for tokens.
///
/// Requires authentication. Validates the CSRF state against the cookie,
/// exchanges the code for tokens, encrypts and stores them.
pub async fn oura_callback(
    State(state): State<AppState>,
    auth_user: AuthUser,
    headers: axum::http::HeaderMap,
    Query(query): Query<OuraCallbackQuery>,
) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .oura_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("OURA_CLIENT_ID not configured".to_string()))?;
    let client_secret = state
        .config
        .oura_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Internal("OURA_CLIENT_SECRET not configured".to_string()))?;

    // Validate CSRF state
    let expected_state = read_cookie(&headers, "oura_oauth_state")
        .ok_or_else(|| ApiError::BadRequest("missing oura_oauth_state cookie".into()))?;

    if expected_state != query.state {
        return Err(ApiError::BadRequest("OAuth state mismatch".into()));
    }

    let redirect_uri = format!("{}/api/v1/auth/oura/callback", state.config.web_origin);

    let client = OuraClient::new(
        client_id.to_string(),
        client_secret.to_string(),
        state.config.oura_api_base_url.clone(),
        state.config.oura_auth_base_url.clone(),
        state.http_client.clone(),
    );

    let tokens = client
        .exchange_code(&query.code, &redirect_uri)
        .await
        .map_err(|e| ApiError::Internal(format!("Oura token exchange failed: {e}")))?;

    let encryption_key = crypto::parse_encryption_key(&state.config.encryption_key)?;

    let expires_at = tokens
        .expires_in
        .map(|secs| Utc::now() + chrono::Duration::seconds(secs));

    integration_tokens::upsert(
        &state.pool,
        auth_user.id,
        "oura",
        &tokens.access_token,
        tokens.refresh_token.as_deref(),
        expires_at,
        &encryption_key,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("failed to store Oura tokens: {e}")))?;

    tracing::info!(user_id = %auth_user.id, "Oura integration connected");

    // Clear the state cookie and redirect to settings.
    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let clear_state =
        format!("oura_oauth_state=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");

    let redirect_url = format!("{}/settings?connected=oura", state.config.web_origin);
    let mut response = Redirect::to(&redirect_url).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        clear_state
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );

    Ok(response)
}

/// Read a named cookie from the request headers.
fn read_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| {
                    let trimmed = c.trim();
                    trimmed
                        .strip_prefix(name)
                        .and_then(|rest| rest.strip_prefix('='))
                        .map(|v| v.to_string())
                })
                .next()
        })
}
