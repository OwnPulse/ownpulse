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
use crate::models::user::{LoginRequest, RefreshRequest, TokenResponse};
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

    issue_tokens(&state, user.id, &user.role).await
}

/// POST /auth/refresh — rotate refresh token, issue new access + refresh.
///
/// Accepts the refresh token from either a JSON body (`{"refresh_token": "..."}`)
/// or an httpOnly cookie. Body takes precedence — iOS uses the body variant since
/// it stores tokens in the Keychain, not cookies.
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<RefreshRequest>>,
) -> Result<Response, ApiError> {
    // Body takes precedence over cookie
    let token_value = if let Some(Json(req)) = body {
        req.refresh_token
    } else {
        let cookie_header = headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        cookie_header
            .split(';')
            .filter_map(|c| {
                let c = c.trim();
                c.strip_prefix("refresh_token=")
            })
            .next()
            .ok_or(ApiError::Unauthorized)?
            .to_string()
    };

    let token_hash = hash_refresh_token(&token_value, &state.config.jwt_secret);

    match refresh_tokens::find_by_hash(&state.pool, &token_hash).await {
        Ok(row) => {
            if row.expires_at < Utc::now() {
                return Err(ApiError::Unauthorized);
            }

            let family_id = row.family_id;
            let user_id = row.user_id;

            // Rotate: delete old token, issue new pair in the same family
            refresh_tokens::delete_by_hash(&state.pool, &token_hash)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            issue_tokens_with_family(&state, user_id, family_id).await
        }
        Err(sqlx::Error::RowNotFound) => {
            // Token not found — possible replay attack. The token was already
            // rotated, meaning an attacker (or stale client) is presenting a
            // used token. Return 401.
            tracing::warn!(
                token_hash_prefix = %&token_hash[..8.min(token_hash.len())],
                "refresh token not found — possible replay attack"
            );
            Err(ApiError::Unauthorized)
        }
        Err(_) => Err(ApiError::Unauthorized),
    }
}

/// POST /auth/logout — revoke the refresh token, clear the cookie.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(token_value) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_header| {
            cookie_header
                .split(';')
                .filter_map(|c| c.trim().strip_prefix("refresh_token="))
                .next()
        })
    {
        let token_hash = hash_refresh_token(token_value, &state.config.jwt_secret);
        // On logout, revoke the entire family to invalidate all related tokens
        if let Ok(row) = refresh_tokens::find_by_hash(&state.pool, &token_hash).await {
            let _ = refresh_tokens::delete_family(&state.pool, row.family_id).await;
        }
    }

    let clear_cookie =
        "refresh_token=; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age=0";

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        clear_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(response)
}

/// GET /auth/google/login — generate OAuth authorization URL with CSRF state.
pub async fn google_login(State(state): State<AppState>) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not configured".to_string()))?;
    let redirect_uri = state
        .config
        .google_redirect_uri
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_REDIRECT_URI not configured".to_string()))?;

    let csrf_state = Uuid::new_v4().to_string();

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope=openid%20email%20profile\
         &state={}",
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&csrf_state),
    );

    let state_cookie = format!(
        "oauth_state={}; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age=600",
        csrf_state
    );

    let mut response = Redirect::to(&auth_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        state_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: String,
    /// CSRF state parameter — validated against the `oauth_state` cookie.
    /// When set to `"ios"`, redirect to `ownpulse://auth#token=...` instead of
    /// the web origin. The iOS app passes `state=ios` in the OAuth URL.
    pub state: Option<String>,
}

/// GET /auth/google/callback?code=...&state=... — exchange authorization code,
/// find/create user, set httpOnly cookies or redirect to iOS.
pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<Response, ApiError> {
    // --- CSRF state validation ---
    let oauth_state_cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| c.trim().strip_prefix("oauth_state="))
                .next()
                .map(|s| s.to_string())
        });

    // iOS uses state=ios and doesn't go through our google_login endpoint,
    // so it won't have the CSRF cookie. For web flows, validate CSRF state.
    let is_ios = query.state.as_deref() == Some("ios");

    if !is_ios {
        let expected_state = oauth_state_cookie
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("missing oauth_state cookie".into()))?;
        let actual_state = query
            .state
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("missing state parameter".into()))?;
        if expected_state != actual_state {
            return Err(ApiError::BadRequest("OAuth state mismatch".into()));
        }
    }

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
        &state.config.google_token_url,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let google_user = crate::integrations::google::fetch_user_info(
        &state.http_client,
        &tokens.access_token,
        &state.config.google_userinfo_url,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let username = sanitize_username(
        google_user
            .email
            .split('@')
            .next()
            .unwrap_or("user"),
    );

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
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Clear the oauth_state cookie
    let clear_state_cookie =
        "oauth_state=; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age=0";

    if is_ios {
        // iOS: redirect to custom scheme with tokens in the fragment
        let redirect_url = format!(
            "ownpulse://auth#token={}&refresh_token={}",
            access_token, raw_token
        );
        let mut response = Redirect::to(&redirect_url).into_response();
        response.headers_mut().append(
            SET_COOKIE,
            clear_state_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
        Ok(response)
    } else {
        // Web: set tokens as httpOnly cookies and redirect without tokens in URL
        let access_cookie = format!(
            "access_token={}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={}",
            access_token, state.config.jwt_expiry_seconds
        );
        let refresh_cookie = format!(
            "refresh_token={}; HttpOnly; Secure; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
            raw_token, state.config.refresh_token_expiry_seconds
        );

        let redirect_url = format!("{}/?auth=success", state.config.web_origin);
        let mut response = Redirect::to(&redirect_url).into_response();

        response.headers_mut().append(
            SET_COOKIE,
            access_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
        response.headers_mut().append(
            SET_COOKIE,
            refresh_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
        response.headers_mut().append(
            SET_COOKIE,
            clear_state_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
        Ok(response)
    }
}

/// Sanitize a username derived from an email local part.
///
/// - Keeps only alphanumeric characters, hyphens, and underscores
/// - Truncates to 32 characters
/// - Falls back to a UUID-based name if empty after sanitization
fn sanitize_username(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();

    if sanitized.is_empty() {
        format!("user-{}", &Uuid::new_v4().to_string()[..8])
    } else {
        sanitized
    }
}

/// Create a JWT access token and a refresh token, returning a JSON body with
/// the access token and setting an httpOnly cookie for the refresh token.
async fn issue_tokens(state: &AppState, user_id: Uuid, role: &str) -> Result<Response, ApiError> {
    let access_token = encode_access_token(
        user_id,
        role,
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
    response.headers_mut().insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(response)
}

/// Issue tokens inheriting an existing refresh-token family (used during rotation).
async fn issue_tokens_with_family(
    state: &AppState,
    user_id: Uuid,
    family_id: Uuid,
) -> Result<Response, ApiError> {
    let user = users::find_by_id(&state.pool, user_id)
        .await
        .map_err(|_| ApiError::Unauthorized)?;
    let access_token = encode_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&raw_refresh, &state.config.jwt_secret);
    let expires_at = Utc::now()
        + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert_with_family(&state.pool, user_id, &refresh_hash, expires_at, family_id)
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
    response.headers_mut().insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_normal_username() {
        assert_eq!(sanitize_username("john.doe"), "johndoe");
    }

    #[test]
    fn sanitize_with_special_chars() {
        assert_eq!(sanitize_username("user+tag@"), "usertag");
    }

    #[test]
    fn sanitize_preserves_hyphens_and_underscores() {
        assert_eq!(sanitize_username("my-user_name"), "my-user_name");
    }

    #[test]
    fn sanitize_truncates_long_names() {
        let long = "a".repeat(50);
        assert_eq!(sanitize_username(&long).len(), 32);
    }

    #[test]
    fn sanitize_empty_falls_back() {
        let result = sanitize_username("...");
        assert!(result.starts_with("user-"));
        assert_eq!(result.len(), 13); // "user-" + 8 hex chars
    }
}
