// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::auth::jwt::encode_access_token;
use crate::auth::refresh::{generate_refresh_token, hash_refresh_token};
use crate::db::invites;
use crate::db::refresh_tokens;
use crate::db::user_auth_methods;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::{
    AppleCallbackRequest, AuthMethodRow, LinkAuthRequest, LoginRequest, RefreshRequest,
    RegisterRequest, TokenResponse, TokenResponseWithRefresh,
};

/// Return `"; Secure"` when the web origin uses HTTPS, empty string otherwise.
/// This lets cookies work over plain HTTP during local development while
/// remaining secure in production.
fn secure_attr(config: &crate::config::Config) -> &'static str {
    if config.web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    }
}

/// Dummy bcrypt hash used when a user is not found, so the response time is
/// indistinguishable from a wrong-password attempt (prevents email enumeration).
const DUMMY_HASH: &str = "$2b$12$K4Q3e1qZ0r3pYh5v5g5X5e5X5e5X5e5X5e5X5e5X5e5X5e5X5e";

/// POST /auth/login — email + password authentication.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    // Basic email format validation
    if body.email.len() > 254 || !body.email.contains('@') {
        // Still run dummy bcrypt to prevent timing leak
        let _ = bcrypt::verify(&body.password, DUMMY_HASH);
        return Err(ApiError::Unauthorized);
    }

    let user = match users::find_by_email(&state.pool, &body.email).await {
        Ok(u) => u,
        Err(_) => {
            // User not found — run bcrypt against a dummy hash so the response
            // time matches a wrong-password attempt (prevents email enumeration).
            let _ = bcrypt::verify(&body.password, DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        }
    };

    let password_hash = user.password_hash.as_deref().unwrap_or(DUMMY_HASH);

    let valid = bcrypt::verify(&body.password, password_hash).unwrap_or(false);
    if !valid {
        return Err(ApiError::Unauthorized);
    }

    if user.status != "active" {
        // Disabled users get a short-lived access token only (no refresh token,
        // no refresh cookie). This lets them reach export and self-delete routes
        // before the token expires.
        return issue_access_token_only(&state, user.id, &user.role).await;
    }

    issue_tokens(&state, user.id, &user.role).await
}

/// POST /auth/register — create a new user with email + password.
///
/// When `require_invite` is true (the default), a valid invite code must be
/// provided. The invite claim and user creation happen inside a single
/// transaction to prevent TOCTOU races.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Response, ApiError> {
    // Validate email
    if body.email.len() > 254 || !body.email.contains('@') {
        return Err(ApiError::BadRequest("invalid email address".into()));
    }

    // Validate password
    if body.password.len() < 8 {
        return Err(ApiError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }

    // Hash password before starting the transaction (bcrypt is slow by design)
    let password_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let username = body
        .username
        .as_deref()
        .map(sanitize_username)
        .unwrap_or_else(|| sanitize_username(body.email.split('@').next().unwrap_or("user")));

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Validate and claim invite code if required
    let claimed_invite = if state.config.require_invite {
        let code = body
            .invite_code
            .as_deref()
            .ok_or_else(|| ApiError::BadRequest("invite code required".into()))?;

        let invite = invites::claim_invite_code_tx(&mut tx, code)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    ApiError::BadRequest("invalid or expired invite code".into())
                }
                other => ApiError::Internal(other.to_string()),
            })?;
        Some(invite)
    } else {
        None
    };

    // Create the user inside the same transaction
    let user = sqlx::query_as::<_, crate::models::user::UserRow>(
        "INSERT INTO users (email, username, password_hash, auth_provider)
         VALUES ($1, $2, $3, 'local')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(&body.email)
    .bind(&username)
    .bind(&password_hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("23505") => {
            ApiError::Conflict("email already registered".into())
        }
        _ => ApiError::Internal(e.to_string()),
    })?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'local', $2, $3)",
    )
    .bind(user.id)
    .bind(user.id.to_string())
    .bind(&body.email)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Record the invite claim audit trail
    if let Some(invite) = claimed_invite {
        invites::record_invite_claim(&mut tx, invite.id, user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

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

    let secure = secure_attr(&state.config);
    let clear_cookie =
        format!("refresh_token=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        clear_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleLoginQuery {
    pub invite_code: Option<String>,
}

/// GET /auth/google/login — generate OAuth authorization URL with CSRF state.
///
/// Accepts an optional `?invite_code=` query param. When provided, the code is
/// stored in a short-lived httpOnly cookie so the callback can use it for new
/// user registration.
pub async fn google_login(
    State(state): State<AppState>,
    Query(login_query): Query<GoogleLoginQuery>,
) -> Result<Response, ApiError> {
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

    let secure = secure_attr(&state.config);
    let state_cookie = format!(
        "oauth_state={csrf_state}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600"
    );

    let mut response = Redirect::to(&auth_url).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        state_cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );

    // Store invite code in a short-lived cookie if provided (alphanumeric only).
    if let Some(ref code) = login_query.invite_code
        && !code.is_empty()
        && code.chars().all(|c| c.is_alphanumeric())
    {
        let invite_cookie = format!(
            "invite_code={code}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=600"
        );
        response.headers_mut().append(
            SET_COOKIE,
            invite_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
    }

    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: String,
    /// CSRF state parameter — validated against the `oauth_state` cookie in web flows.
    /// Not required when `code_verifier` is present (PKCE flow).
    pub state: Option<String>,
    /// PKCE code verifier (RFC 7636) — native app flows send this instead of relying
    /// on a CSRF cookie. Google validates it against the `code_challenge` sent during
    /// authorization. When present, the `oauth_state` cookie check is skipped because
    /// possession of the verifier proves the caller initiated the flow.
    pub code_verifier: Option<String>,
}

/// GET /auth/google/callback?code=...&state=... — exchange authorization code,
/// find/create user, set httpOnly cookies or redirect to iOS.
pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<Response, ApiError> {
    // --- CSRF / PKCE validation ---
    //
    // Two mutually exclusive flows are supported:
    //
    // 1. PKCE (native app): the client sends `code_verifier`; Google will
    //    validate it against the `code_challenge` that was included in the
    //    original authorization URL. No CSRF cookie is needed because
    //    possession of the verifier cryptographically proves the caller
    //    initiated the flow (RFC 7636 §4.6).
    //
    // 2. Web (browser): no `code_verifier`; we validate the `state` parameter
    //    against the short-lived httpOnly `oauth_state` cookie set by
    //    `google_login`. This is the standard OAuth 2.0 CSRF mitigation.
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

    if query.code_verifier.is_none() {
        // Web flow — validate state parameter against the CSRF cookie.
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
    // PKCE flow — no cookie check here; Google validates the verifier during
    // token exchange and will reject the request if it does not match.

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
        query.code_verifier.as_deref(),
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

    let display_name = sanitize_username(google_user.email.split('@').next().unwrap_or("user"));

    // Extract the invite code cookie once (used when creating new users with
    // require_invite enabled).
    let invite_code_cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| c.trim().strip_prefix("invite_code="))
                .next()
                .map(|s| s.to_string())
        });

    // Always begin a transaction so the existence check, invite claim, and user
    // creation are atomic — prevents TOCTOU races where a concurrent deletion
    // between the check and creation could bypass the invite requirement.
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Check if user already exists inside the transaction.
    let existing_user =
        users::find_google_user_tx(&mut tx, &google_user.sub, &google_user.email).await;

    let user = match existing_user {
        Ok(user) => {
            // Existing user — no invite needed.
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            user
        }
        Err(sqlx::Error::RowNotFound) => {
            // New user — claim invite if required, then create.
            let claimed_invite = if state.config.require_invite {
                let code = invite_code_cookie.ok_or_else(|| {
                    ApiError::BadRequest("invite code required for new account registration".into())
                })?;

                let invite = invites::claim_invite_code_tx(&mut tx, &code)
                    .await
                    .map_err(|e| match e {
                        sqlx::Error::RowNotFound => {
                            ApiError::BadRequest("invalid or expired invite code".into())
                        }
                        other => ApiError::Internal(other.to_string()),
                    })?;
                Some(invite)
            } else {
                None
            };

            let user = users::find_or_create_google_user_tx(
                &mut tx,
                &google_user.sub,
                &google_user.email,
                Some(display_name.as_str()),
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            // Record the invite claim audit trail
            if let Some(invite) = claimed_invite {
                invites::record_invite_claim(&mut tx, invite.id, user.id)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }

            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            user
        }
        Err(e) => return Err(ApiError::Internal(e.to_string())),
    };

    if user.status != "active" {
        // Disabled users get a short-lived access token only (no refresh token,
        // no refresh cookie). This lets them reach export and self-delete routes
        // before the token expires — same behaviour as password login.
        return issue_access_token_only(&state, user.id, &user.role).await;
    }

    // Issue tokens and build the response (shared by both invite and non-invite paths).
    let raw_token = generate_refresh_token();
    let token_hash = hash_refresh_token(&raw_token, &state.config.jwt_secret);
    let expires_at =
        Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

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

    // Clear the oauth_state and invite_code cookies.
    let secure = secure_attr(&state.config);
    let clear_state_cookie =
        format!("oauth_state=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");
    let clear_invite_cookie =
        format!("invite_code=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");

    if query.code_verifier.is_some() {
        // Native app (PKCE flow): redirect to the custom URI scheme with tokens
        // in the URL fragment so the app can extract them from the redirect.
        // The app stores these tokens in the Keychain, never in cookies.
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
        response.headers_mut().append(
            SET_COOKIE,
            clear_invite_cookie
                .parse()
                .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
        );
        Ok(response)
    } else {
        // Web flow: set tokens as httpOnly cookies and redirect without tokens in URL.
        let access_cookie = format!(
            "access_token={access_token}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age={}",
            state.config.jwt_expiry_seconds
        );
        let refresh_cookie = format!(
            "refresh_token={raw_token}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
            state.config.refresh_token_expiry_seconds
        );

        let redirect_url = format!("{}/?auth=success", state.config.web_origin);
        let mut response = Redirect::to(&redirect_url).into_response();

        for cookie_str in [
            &access_cookie,
            &refresh_cookie,
            &clear_state_cookie,
            &clear_invite_cookie,
        ] {
            response.headers_mut().append(
                SET_COOKIE,
                cookie_str
                    .parse()
                    .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
            );
        }
        Ok(response)
    }
}

/// POST /auth/apple/callback — verify Apple id_token and issue tokens.
///
/// For iOS clients (`platform != "web"`) the refresh token is included in the
/// JSON body. For web clients it is set as an httpOnly cookie only.
pub async fn apple_callback(
    State(state): State<AppState>,
    Json(body): Json<AppleCallbackRequest>,
) -> Result<Response, ApiError> {
    // Validate platform against known values.
    match body.platform.as_str() {
        "web" | "ios" => {}
        _ => {
            return Err(ApiError::BadRequest(format!(
                "unknown platform: {}",
                body.platform
            )));
        }
    }

    let client_id = state
        .config
        .apple_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("APPLE_CLIENT_ID not configured".to_string()))?;

    let apple_user = crate::integrations::apple::verify_identity_token(
        &state.http_client,
        &body.id_token,
        client_id,
        &state.config.apple_jwks_url,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "Apple identity token verification failed");
        ApiError::Unauthorized
    })?;

    // Apple may not provide an email (e.g. private relay, or after first sign-in).
    // Generate a placeholder email if needed since the users table requires one.
    let placeholder_email;
    let email = match apple_user.email.as_deref() {
        Some(e) => e,
        None => {
            placeholder_email = format!(
                "{}@privaterelay.appleid.com",
                &apple_user.sub[..8.min(apple_user.sub.len())]
            );
            &placeholder_email
        }
    };
    let username = email
        .split('@')
        .next()
        .map(sanitize_username)
        .unwrap_or_else(|| format!("user-{}", &Uuid::new_v4().to_string()[..8]));

    // Always begin a transaction so the existence check, invite claim, and user
    // creation are atomic — prevents TOCTOU races.
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Check if user already exists *inside* the transaction.
    let existing_user = users::find_apple_user_tx(&mut tx, &apple_user.sub, Some(email)).await;

    let user = match existing_user {
        Ok(user) => {
            // Existing user — no invite needed.
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            user
        }
        Err(sqlx::Error::RowNotFound) => {
            // New user — claim invite if required, then create.
            let claimed_invite = if state.config.require_invite {
                let code = body.invite_code.as_deref().ok_or_else(|| {
                    ApiError::BadRequest("invite code required for new account registration".into())
                })?;

                let invite =
                    invites::claim_invite_code_tx(&mut tx, code)
                        .await
                        .map_err(|e| match e {
                            sqlx::Error::RowNotFound => {
                                ApiError::BadRequest("invalid or expired invite code".into())
                            }
                            other => ApiError::Internal(other.to_string()),
                        })?;
                Some(invite)
            } else {
                None
            };

            let user = users::find_or_create_apple_user_tx(
                &mut tx,
                &apple_user.sub,
                Some(email),
                &username,
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            // Record the invite claim audit trail
            if let Some(invite) = claimed_invite {
                invites::record_invite_claim(&mut tx, invite.id, user.id)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
            }

            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            user
        }
        Err(e) => return Err(ApiError::Internal(e.to_string())),
    };

    if user.status != "active" {
        // Disabled users get a short-lived access token only (no refresh token,
        // no refresh cookie). This lets them reach export and self-delete routes
        // before the token expires — same behaviour as password login.
        return issue_access_token_only(&state, user.id, &user.role).await;
    }

    let is_web = body.platform == "web";
    issue_tokens_response(&state, user.id, &user.role, is_web).await
}

/// POST /auth/link — link a new auth provider to the authenticated user's account.
pub async fn link_auth(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(body): Json<LinkAuthRequest>,
) -> Result<Response, ApiError> {
    match body.provider.as_str() {
        "apple" => {
            let id_token = body
                .id_token
                .as_deref()
                .ok_or_else(|| ApiError::BadRequest("id_token required for apple".into()))?;

            let client_id =
                state.config.apple_client_id.as_deref().ok_or_else(|| {
                    ApiError::Internal("APPLE_CLIENT_ID not configured".to_string())
                })?;

            let apple_user = crate::integrations::apple::verify_identity_token(
                &state.http_client,
                id_token,
                client_id,
                &state.config.apple_jwks_url,
            )
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "Apple identity token verification failed during link");
                ApiError::Unauthorized
            })?;

            // Check that this Apple sub isn't already linked to a DIFFERENT user.
            match user_auth_methods::find_by_provider_subject(&state.pool, "apple", &apple_user.sub)
                .await
            {
                Ok(existing) if existing.id != auth_user.id => {
                    return Err(ApiError::Conflict(
                        "this Apple account is already linked to another user".into(),
                    ));
                }
                Ok(_) => {
                    // Already linked to this user — idempotent, fall through to return list.
                }
                Err(sqlx::Error::RowNotFound) => {
                    user_auth_methods::insert(
                        &state.pool,
                        auth_user.id,
                        "apple",
                        Some(&apple_user.sub),
                        apple_user.email.as_deref(),
                    )
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                }
                Err(e) => return Err(ApiError::Internal(e.to_string())),
            }
        }
        "local" => {
            let password = body
                .password
                .as_deref()
                .ok_or_else(|| ApiError::BadRequest("password required for local".into()))?;

            if password.len() < 8 {
                return Err(ApiError::BadRequest(
                    "password must be at least 8 characters".into(),
                ));
            }

            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            // local uses user_id as provider_subject
            match user_auth_methods::find_by_provider_subject(
                &state.pool,
                "local",
                &auth_user.id.to_string(),
            )
            .await
            {
                Ok(_) => {
                    // Already linked — idempotent.
                }
                Err(sqlx::Error::RowNotFound) => {
                    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
                    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
                        .bind(&hash)
                        .bind(auth_user.id)
                        .execute(&mut *tx)
                        .await
                        .map_err(ApiError::from)?;
                    sqlx::query(
                        "INSERT INTO user_auth_methods (user_id, provider, provider_subject)
                         VALUES ($1, 'local', $2)
                         ON CONFLICT DO NOTHING",
                    )
                    .bind(auth_user.id)
                    .bind(auth_user.id.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(ApiError::from)?;
                    tx.commit().await.map_err(ApiError::from)?;
                }
                Err(e) => return Err(ApiError::Internal(e.to_string())),
            }
        }
        "google" => {
            return Err(ApiError::BadRequest(
                "Google account linking is not yet supported; use Google Sign-In to create your account instead".into(),
            ));
        }
        other => {
            return Err(ApiError::BadRequest(format!(
                "unsupported provider: {other}"
            )));
        }
    }

    let methods = user_auth_methods::list_for_user(&state.pool, auth_user.id)
        .await
        .map_err(ApiError::from)?;

    Ok((StatusCode::OK, Json(methods)).into_response())
}

/// DELETE /auth/link/:provider — unlink an auth provider from the user's account.
pub async fn unlink_auth(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(provider): Path<String>,
) -> Result<Response, ApiError> {
    let rows_deleted = user_auth_methods::delete_if_not_last(&state.pool, auth_user.id, &provider)
        .await
        .map_err(ApiError::from)?;

    if rows_deleted == 0 {
        // Distinguish "last method" from "provider not linked":
        // delete_if_not_last returns 0 for both cases.
        let methods = user_auth_methods::list_for_user(&state.pool, auth_user.id)
            .await
            .map_err(ApiError::from)?;
        let provider_exists = methods.iter().any(|m| m.provider == provider);
        if !provider_exists {
            return Err(ApiError::NotFoundMsg("provider not linked".into()));
        }
        return Err(ApiError::BadRequest(
            "cannot remove your only login method".into(),
        ));
    }

    let methods = user_auth_methods::list_for_user(&state.pool, auth_user.id)
        .await
        .map_err(ApiError::from)?;

    Ok((StatusCode::OK, Json(methods)).into_response())
}

/// GET /auth/methods — list all auth methods linked to the current user.
pub async fn list_auth_methods(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<AuthMethodRow>>, ApiError> {
    let methods = user_auth_methods::list_for_user(&state.pool, auth_user.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(methods))
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

/// Issue only a short-lived JWT access token — no refresh token, no cookie.
///
/// Used for disabled users who are allowed to log in only to export their data
/// or delete their account. Without a refresh token they cannot extend the
/// session beyond the access token's lifetime.
async fn issue_access_token_only(
    state: &AppState,
    user_id: Uuid,
    role: &str,
) -> Result<Response, ApiError> {
    let access_token = encode_access_token(
        user_id,
        role,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let token_response = TokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.config.jwt_expiry_seconds,
    };

    Ok((StatusCode::OK, Json(token_response)).into_response())
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
    let expires_at =
        Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert(&state.pool, user_id, &refresh_hash, expires_at)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let secure = secure_attr(&state.config);
    let cookie = format!(
        "refresh_token={raw_refresh}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        state.config.refresh_token_expiry_seconds
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

    if user.status != "active" {
        return Err(ApiError::Forbidden);
    }

    let access_token = encode_access_token(
        user_id,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&raw_refresh, &state.config.jwt_secret);
    let expires_at =
        Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert_with_family(&state.pool, user_id, &refresh_hash, expires_at, family_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let secure = secure_attr(&state.config);
    let cookie = format!(
        "refresh_token={raw_refresh}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        state.config.refresh_token_expiry_seconds
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

/// Issue tokens and return the response appropriate for the platform.
///
/// For web clients: refresh token in httpOnly cookie only.
/// For iOS / non-web clients: refresh token in JSON body + httpOnly cookie.
async fn issue_tokens_response(
    state: &AppState,
    user_id: Uuid,
    role: &str,
    is_web: bool,
) -> Result<Response, ApiError> {
    let access_token = encode_access_token(
        user_id,
        role,
        &state.config.jwt_secret,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&raw_refresh, &state.config.jwt_secret);
    let expires_at =
        Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);

    refresh_tokens::insert(&state.pool, user_id, &refresh_hash, expires_at)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let secure = secure_attr(&state.config);
    let cookie = format!(
        "refresh_token={raw_refresh}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        state.config.refresh_token_expiry_seconds
    );

    let mut response = if is_web {
        // Web: return access token in body, refresh token in cookie only.
        let token_response = TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: state.config.jwt_expiry_seconds,
        };
        (StatusCode::OK, Json(token_response)).into_response()
    } else {
        // iOS: include refresh token in JSON body so the client can store it
        // in the Keychain without relying on cookies.
        let token_response = TokenResponseWithRefresh {
            access_token,
            refresh_token: raw_refresh,
            token_type: "Bearer".to_string(),
            expires_in: state.config.jwt_expiry_seconds,
        };
        (StatusCode::OK, Json(token_response)).into_response()
    };

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
