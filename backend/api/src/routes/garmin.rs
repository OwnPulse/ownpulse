// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Garmin OAuth 1.0a flow — connect and callback routes.

use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::crypto;
use crate::db::integration_tokens;
use crate::error::ApiError;
use crate::integrations::garmin::GarminClient;

/// GET /auth/garmin/login — start the OAuth 1.0a flow.
///
/// Requires authentication. Obtains a request token from Garmin and redirects
/// the user to Garmin's authorization page. The request token secret is stored
/// in a short-lived httpOnly cookie for the callback to use.
pub async fn garmin_login(
    State(state): State<AppState>,
    AuthUser { .. }: AuthUser,
) -> Result<Response, ApiError> {
    let consumer_key = state
        .config
        .garmin_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GARMIN_CLIENT_ID not configured".to_string()))?;
    let consumer_secret = state
        .config
        .garmin_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GARMIN_CLIENT_SECRET not configured".to_string()))?;

    let callback_url = format!("{}/api/v1/auth/garmin/callback", state.config.web_origin);

    let client = GarminClient::new(
        consumer_key.to_string(),
        consumer_secret.to_string(),
        state.config.garmin_base_url.clone(),
        state.http_client.clone(),
    );

    let request_token = client
        .get_request_token(&callback_url)
        .await
        .map_err(|e| ApiError::Internal(format!("Garmin request token failed: {e}")))?;

    let auth_url = client.authorization_url(&request_token.oauth_token);

    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };

    // Store the request token secret in a cookie so the callback can use it
    // to exchange for an access token. Also store the token itself to verify
    // it matches what comes back.
    let secret_cookie = format!(
        "garmin_oauth_secret={}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600",
        request_token.oauth_token_secret
    );
    let token_cookie = format!(
        "garmin_oauth_token={}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600",
        request_token.oauth_token
    );

    let mut response = Redirect::to(&auth_url).into_response();
    for cookie in [&secret_cookie, &token_cookie] {
        response.headers_mut().append(
            SET_COOKIE,
            cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
    }

    Ok(response)
}

#[derive(Deserialize)]
pub struct GarminCallbackQuery {
    pub oauth_token: String,
    pub oauth_verifier: String,
}

/// GET /auth/garmin/callback — exchange the verifier for an access token.
///
/// Requires authentication. Reads the request token secret from the cookie,
/// exchanges for an access token, encrypts and stores it in integration_tokens.
pub async fn garmin_callback(
    State(state): State<AppState>,
    auth_user: AuthUser,
    headers: axum::http::HeaderMap,
    Query(query): Query<GarminCallbackQuery>,
) -> Result<Response, ApiError> {
    let consumer_key = state
        .config
        .garmin_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GARMIN_CLIENT_ID not configured".to_string()))?;
    let consumer_secret = state
        .config
        .garmin_client_secret
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GARMIN_CLIENT_SECRET not configured".to_string()))?;

    // Read the request token secret from the cookie set during garmin_login.
    let request_token_secret = read_cookie(&headers, "garmin_oauth_secret")
        .ok_or_else(|| ApiError::BadRequest("missing garmin_oauth_secret cookie".into()))?;

    let stored_token = read_cookie(&headers, "garmin_oauth_token")
        .ok_or_else(|| ApiError::BadRequest("missing garmin_oauth_token cookie".into()))?;

    // Verify the token matches what we stored.
    if stored_token != query.oauth_token {
        return Err(ApiError::BadRequest(
            "OAuth token mismatch — possible CSRF".into(),
        ));
    }

    let client = GarminClient::new(
        consumer_key.to_string(),
        consumer_secret.to_string(),
        state.config.garmin_base_url.clone(),
        state.http_client.clone(),
    );

    let access_token = client
        .get_access_token(
            &query.oauth_token,
            &request_token_secret,
            &query.oauth_verifier,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Garmin access token exchange failed: {e}")))?;

    // Garmin OAuth 1.0a tokens are a pair: oauth_token + oauth_token_secret.
    // Store the access token as the main token and the secret as the refresh token
    // (Garmin tokens don't expire, but we need both to sign requests).
    let encryption_key = crypto::parse_encryption_key(&state.config.encryption_key)?;

    integration_tokens::upsert(
        &state.pool,
        auth_user.id,
        "garmin",
        &access_token.oauth_token,
        Some(&access_token.oauth_token_secret),
        None, // Garmin tokens don't expire
        &encryption_key,
    )
    .await
    .map_err(|e| ApiError::Internal(format!("failed to store Garmin tokens: {e}")))?;

    tracing::info!(user_id = %auth_user.id, "Garmin integration connected");

    // Clear the temporary cookies and redirect to settings.
    let secure = if state.config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    let clear_secret =
        format!("garmin_oauth_secret=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");
    let clear_token =
        format!("garmin_oauth_token=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");

    let redirect_url = format!("{}/settings?connected=garmin", state.config.web_origin);
    let mut response = Redirect::to(&redirect_url).into_response();
    for cookie in [&clear_secret, &clear_token] {
        response.headers_mut().append(
            SET_COOKIE,
            cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
    }

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
