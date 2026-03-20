// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::{Query, State};
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::jwt::encode_access_token;
use crate::auth::refresh::{generate_refresh_token, hash_refresh_token};
use crate::db::refresh_tokens;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::{LoginRequest, TokenResponse};
use crate::AppState;

/// POST /auth/login — username + password authentication.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let user = users::find_by_username(&state.pool, &body.username)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    let password_hash = user.password_hash.as_deref().ok_or(ApiError::Unauthorized)?;

    let valid =
        bcrypt::verify(&body.password, password_hash).map_err(|_| ApiError::Unauthorized)?;
    if !valid {
        return Err(ApiError::Unauthorized);
    }

    issue_tokens(&state, user.id).await
}

/// POST /auth/refresh — rotate refresh token, issue new access + refresh.
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let token_value = cookie_header
        .split(';')
        .filter_map(|c| {
            let c = c.trim();
            c.strip_prefix("refresh_token=")
        })
        .next()
        .ok_or(ApiError::Unauthorized)?;

    let token_hash = hash_refresh_token(token_value, &state.config.jwt_secret);
    let row = refresh_tokens::find_by_hash(&state.pool, &token_hash)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    if row.expires_at < Utc::now() {
        return Err(ApiError::Unauthorized);
    }

    // Rotate: delete old token, issue new pair
    refresh_tokens::delete_by_hash(&state.pool, &token_hash)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    issue_tokens(&state, row.user_id).await
}

/// POST /auth/logout — revoke the refresh token, clear the cookie.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(cookie_header) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token_value) = cookie_header
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("refresh_token="))
            .next()
        {
            let token_hash = hash_refresh_token(token_value, &state.config.jwt_secret);
            let _ = refresh_tokens::delete_by_hash(&state.pool, &token_hash).await;
        }
    }

    let clear_cookie =
        "refresh_token=; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age=0";

    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, clear_cookie.parse().unwrap());
    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: String,
}

/// GET /auth/google/callback?code=... — exchange authorization code, find/create user, redirect.
pub async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<GoogleCallbackQuery>,
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
    let redirect_uri = state
        .config
        .google_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_REDIRECT_URI not configured".to_string()))?;

    let tokens = crate::integrations::google::exchange_code_for_tokens(
        &state.http_client,
        client_id,
        client_secret,
        redirect_uri,
        &query.code,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let google_user = crate::integrations::google::fetch_user_info(
        &state.http_client,
        &tokens.access_token,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let username = google_user
        .email
        .split('@')
        .next()
        .unwrap_or("user")
        .to_string();

    let user = users::find_or_create_google_user(&state.pool, &google_user.email, &username)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Issue tokens
    let raw_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&raw_token, &state.config.jwt_secret);
    let expires_at = Utc::now()
        + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert(&state.pool, user.id, &token_hash, expires_at)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let access_token = encode_access_token(
        user.id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let cookie = format!(
        "refresh_token={}; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        raw_token, state.config.refresh_token_expiry_seconds
    );

    let redirect_url = format!("{}/?token={}", state.config.web_origin, access_token);

    let mut response = Redirect::to(&redirect_url).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.parse().unwrap());
    Ok(response)
}

/// Create a JWT access token and a refresh token, returning a JSON body with
/// the access token and setting an httpOnly cookie for the refresh token.
async fn issue_tokens(state: &AppState, user_id: Uuid) -> Result<Response, ApiError> {
    let access_token = encode_access_token(
        user_id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&raw_refresh, &state.config.jwt_secret);
    let expires_at = Utc::now()
        + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert(&state.pool, user_id, &refresh_hash, expires_at)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let cookie = format!(
        "refresh_token={}; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        raw_refresh, state.config.refresh_token_expiry_seconds
    );

    let token_response = TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.config.jwt_expiry_seconds,
    };

    let mut response = (StatusCode::OK, Json(token_response)).into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie.parse().unwrap());
    Ok(response)
}
