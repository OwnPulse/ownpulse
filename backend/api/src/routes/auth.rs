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
use crate::auth::jwt::{decode_access_token, encode_access_token};
use crate::auth::refresh::{generate_refresh_token, hash_refresh_token};
use crate::crypto;
use crate::db::invites;
use crate::db::password_reset_tokens;
use crate::db::refresh_tokens;
use crate::db::user_auth_methods;
use crate::db::users;
use crate::error::ApiError;
use crate::models::user::{
    AppleCallbackRequest, AuthMethodRow, ForgotPasswordRequest, LinkAuthRequest, LoginRequest,
    RefreshRequest, RegisterRequest, ResetPasswordRequest, TokenResponse, TokenResponseWithRefresh,
};
use crate::routes::read_cookie;

/// Return `"; Secure"` when the web origin uses HTTPS, empty string otherwise.
/// This lets cookies work over plain HTTP during local development while
/// remaining secure in production.
fn secure_attr(web_origin: &str) -> &'static str {
    if web_origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    }
}

/// Name of a cookie that must be host-only. Carries the `__Host-` prefix
/// when the origin is HTTPS: no sibling subdomain can set such a cookie, so
/// an injected value cannot impersonate an account or a flow in the
/// browser-redirect handlers that read it. The prefix also demands `Secure`
/// and `Path=/`, and a plain-HTTP origin cannot satisfy `Secure`, so those
/// keep the bare name — `secure_attr` gates on the same condition, and the
/// two must agree or the browser rejects every cookie we set. Every caller
/// must set the cookie with `Path=/`.
fn host_cookie_name(web_origin: &str, base: &str) -> String {
    if secure_attr(web_origin).is_empty() {
        base.to_string()
    } else {
        format!("__Host-{base}")
    }
}

/// Extract a user ID from the access-token httpOnly cookie. Only validates
/// the JWT (signature, algorithm, expiry) — does NOT check DB status.
fn extract_user_id_from_cookie(
    headers: &HeaderMap,
    config: &crate::config::Config,
) -> Option<Uuid> {
    read_cookie(
        headers,
        &host_cookie_name(&config.web_origin, "access_token"),
    )
    .and_then(|token| decode_access_token(&token, &config.jwt_secret, &config.web_origin).ok())
    .map(|claims| claims.sub)
}

/// Append a Set-Cookie header to a response.
fn append_cookie(response: &mut Response, cookie: &str) -> Result<(), ApiError> {
    response.headers_mut().append(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Internal("failed to build cookie header".into()))?,
    );
    Ok(())
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
    if body.password.len() < 10 {
        return Err(ApiError::BadRequest(
            "password must be at least 10 characters".into(),
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

    // Skip invite requirement when this is the very first user (bootstrap).
    let is_first_user = users::is_empty_tx(&mut tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // If first user, acquire advisory lock and re-check to prevent TOCTOU race
    let is_first_user = if is_first_user {
        users::acquire_bootstrap_lock_tx(&mut tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        // Re-check after acquiring lock — another request may have created a user
        users::is_empty_tx(&mut tx)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        false
    };

    if is_first_user {
        tracing::info!("first user registration — invite requirement bypassed");
    }

    // Validate and claim invite code if required
    let claimed_invite = if state.config.require_invite && !is_first_user {
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

    // Promote first user to admin so they can create invite codes
    let role = if is_first_user {
        users::promote_to_admin_tx(&mut tx, user.id)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        tracing::info!(user_id = %user.id, "first user promoted to admin");
        "admin"
    } else {
        &user.role
    };

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    issue_tokens(&state, user.id, role).await
}

/// How long a rotated refresh token stays presentable. Web tabs share one
/// httpOnly cookie, so concurrent refreshes race; within this window every
/// caller receives the same successor instead of a 401 logout. Reuse after
/// the window is treated as theft and revokes the token family. Keep this
/// well under `REFRESH_TOKEN_EXPIRY_SECONDS` — a large value disables reuse
/// detection. `rotated_at` is written with Postgres `now()` and compared
/// against app-side time; 60s dwarfs any realistic clock skew.
const REFRESH_ROTATE_GRACE_SECONDS: i64 = 60;

/// How many distinct `refresh_token` cookies to examine. A browser holds at
/// most a couple legitimately (ours, plus one a sibling subdomain set);
/// anything beyond that is a client packing the header to multiply the
/// per-request database lookups below.
const MAX_REFRESH_COOKIES: usize = 3;

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
    // Body takes precedence over cookie. Track which source provided the
    // token so we can shape the response appropriately: web clients use the
    // httpOnly cookie and ignore JSON refresh fields; native clients (iOS)
    // send the token in the body and need the rotated refresh token back in
    // the body so they can persist it to Keychain.
    let (token_value, is_web) = if let Some(Json(req)) = body {
        (req.refresh_token, false)
    } else {
        let cookie_header = headers
            .get(axum::http::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        // A sibling subdomain can set a `Domain`-scoped cookie of the same
        // name, which the browser sends alongside ours, and RFC 6265 orders
        // longer paths first — so an attacker picks what a first-match read
        // would return. The `__Host-` prefix that prevents this outright
        // requires `Path=/` (see docs/security.md).
        //
        // Disambiguate on validity rather than count: a fixation attempt
        // needs its injected token to be live, so two live tokens is the
        // attack, while a stale or junk duplicate — the common case, and
        // one the user cannot clear themselves — must not lock them out.
        let mut candidates: Vec<String> = cookie_header
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("refresh_token="))
            .map(str::to_string)
            .collect();
        candidates.sort();
        candidates.dedup();
        candidates.truncate(MAX_REFRESH_COOKIES);

        let mut live: Vec<String> = Vec::new();
        for candidate in &candidates {
            let hash = hash_refresh_token(candidate, &state.config.jwt_secret);
            if refresh_tokens::find_by_hash(&state.pool, &hash)
                .await
                .is_ok()
            {
                live.push(candidate.clone());
            }
        }
        if live.len() > 1 {
            tracing::warn!(
                count = live.len(),
                "multiple live refresh_token cookies presented — possible session fixation"
            );
            return Err(ApiError::Unauthorized);
        }

        // With no live candidate, fall through on the first value so the
        // unknown-token path reports it as usual.
        let cookie_value = live
            .into_iter()
            .next()
            .or_else(|| candidates.into_iter().next())
            .ok_or(ApiError::Unauthorized)?;

        (cookie_value, true)
    };

    let token_hash = hash_refresh_token(&token_value, &state.config.jwt_secret);
    let hash_prefix = &token_hash[..8.min(token_hash.len())];

    // The row lock serializes concurrent refreshes of this token and blocks
    // a concurrent family revocation (logout) until the rotation commits —
    // a revoked family can never be resurrected by an in-flight refresh.
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let row = refresh_tokens::find_by_hash_for_update(&mut tx, &token_hash)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let Some(row) = row else {
        // Unknown hash: most commonly a stale tab refreshing after its
        // family was revoked (logout). Not an attack signal by itself.
        tracing::info!(
            token_hash_prefix = %hash_prefix,
            "unknown refresh token (revoked or never issued)"
        );
        return Err(ApiError::Unauthorized);
    };

    let family_id = row.family_id;
    let user_id = row.user_id;
    let now = Utc::now();
    let grace = chrono::Duration::seconds(REFRESH_ROTATE_GRACE_SECONDS);

    // Use the transaction's connection — a second pool acquire while this
    // one is pinned can exhaust the pool under a multi-tab refresh burst.
    let user = users::find_by_id_tx(&mut tx, user_id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ApiError::Unauthorized,
            other => ApiError::Internal(other.to_string()),
        })?;
    if user.status != "active" {
        return Err(ApiError::Forbidden);
    }

    if row.expires_at < now {
        // An expired rotated token presented after grace is the same theft
        // signal as a live one — don't lose it to the expiry check.
        if row.rotated_at.is_some_and(|t| now - t > grace) {
            revoke_family_in_tx(&state, tx, family_id).await?;
            tracing::warn!(
                %user_id,
                %family_id,
                token_hash_prefix = %hash_prefix,
                "expired refresh token reused after grace window — family revoked"
            );
        }
        return Err(ApiError::Unauthorized);
    }

    match row.rotated_at {
        None => {
            // Active token: rotate. Mint the successor, persist it and its
            // encrypted copy for grace replays, all under the row lock.
            let raw_refresh = generate_refresh_token();
            let refresh_hash = hash_refresh_token(&raw_refresh, &state.config.jwt_secret);
            let expires_at =
                now + chrono::Duration::seconds(state.config.refresh_token_expiry_seconds as i64);
            refresh_tokens::insert_with_family_tx(
                &mut tx,
                user_id,
                &refresh_hash,
                expires_at,
                family_id,
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            let key = crypto::parse_encryption_key(&state.config.encryption_key)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            let ciphertext = crypto::encrypt(&raw_refresh, &key)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            refresh_tokens::mark_rotated_tx(&mut tx, &token_hash, &ciphertext)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            if let Err(e) =
                refresh_tokens::cleanup_family(&state.pool, family_id, REFRESH_ROTATE_GRACE_SECONDS)
                    .await
            {
                tracing::warn!(%family_id, error = %e, "refresh token family cleanup failed");
            }

            rotation_response(&state, &user, raw_refresh, is_web)
        }
        Some(t) if now - t <= grace => {
            // A concurrent refresh already rotated this token — normal for
            // web tabs sharing one cookie. Return the SAME successor: a
            // shared chain keeps a thief and the legitimate client
            // colliding, so post-grace reuse detection still fires.
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            let Some(ciphertext) = row.successor_ciphertext else {
                tracing::warn!(
                    %user_id,
                    %family_id,
                    "rotated refresh token has no stored successor — rejecting"
                );
                return Err(ApiError::Unauthorized);
            };
            let key = crypto::parse_encryption_key(&state.config.encryption_key)
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            let previous = state
                .config
                .encryption_key_previous
                .as_deref()
                .and_then(|k| crypto::parse_encryption_key(k).ok());
            let raw_refresh = crypto::decrypt(&ciphertext, &key, previous.as_ref())
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            tracing::info!(
                %user_id,
                %family_id,
                token_hash_prefix = %hash_prefix,
                "refresh token replayed within grace — returning shared successor"
            );
            rotation_response(&state, &user, raw_refresh, is_web)
        }
        Some(_) => {
            // Reuse well after rotation: assume the token was stolen and
            // revoke every token in the family.
            revoke_family_in_tx(&state, tx, family_id).await?;
            tracing::warn!(
                %user_id,
                %family_id,
                token_hash_prefix = %hash_prefix,
                "refresh token reused after grace window — family revoked"
            );
            Err(ApiError::Unauthorized)
        }
    }
}

/// Revoke a family from inside the refresh handler's transaction (running it
/// on the pool would deadlock against the handler's own row lock), then run
/// the pool-side passes that catch successors a concurrent rotation slipped
/// past the transactional DELETE's snapshot.
async fn revoke_family_in_tx(
    state: &AppState,
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    family_id: Uuid,
) -> Result<(), ApiError> {
    refresh_tokens::delete_family_tx(&mut tx, family_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if let Err(e) = refresh_tokens::delete_family(&state.pool, family_id).await {
        tracing::warn!(%family_id, error = %e, "post-revocation family sweep failed");
    }
    Ok(())
}

/// POST /auth/logout — revoke the refresh token, clear the cookie.
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    // Revoke every presented token rather than the first: if a second
    // refresh_token cookie is in play (a sibling subdomain can set one),
    // logging out must end the real session, not whichever cookie happened
    // to be parsed first.
    let mut presented: Vec<String> = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|cookie_header| {
            cookie_header
                .split(';')
                .filter_map(|c| c.trim().strip_prefix("refresh_token="))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    presented.sort();
    presented.dedup();

    for token_value in presented.into_iter().take(MAX_REFRESH_COOKIES) {
        let token_hash = hash_refresh_token(&token_value, &state.config.jwt_secret);
        // On logout, revoke the entire family to invalidate all related tokens
        if let Ok(row) = refresh_tokens::find_by_hash(&state.pool, &token_hash).await {
            let _ = refresh_tokens::delete_family(&state.pool, row.family_id).await;
        }
    }

    let secure = secure_attr(&state.config.web_origin);
    let clear_refresh =
        format!("refresh_token=; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age=0");
    // Clear the access token too: it outlives logout by up to its expiry,
    // and the Google link flow reads it to decide which account a provider
    // identity attaches to — so leaving it behind keeps a signed-out
    // browser able to act as that user.
    let clear_access = format!(
        "{}=; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age=0",
        host_cookie_name(&state.config.web_origin, "access_token")
    );

    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut response, &clear_refresh)?;
    append_cookie(&mut response, &clear_access)?;
    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleLoginQuery {
    pub invite_code: Option<String>,
    /// When `mode=link`, the OAuth flow will link Google to the authenticated
    /// user's account instead of logging in / registering.
    pub mode: Option<String>,
    /// When `platform=ios`, the callback will redirect to the `ownpulse://`
    /// custom URI scheme instead of the web origin.
    pub platform: Option<String>,
}

/// GET /auth/google/login — generate OAuth authorization URL with CSRF state.
///
/// Everything the callback needs beyond Google's authorization code — the
/// platform, the invite code, and whether this is a link flow (and for whom)
/// — is recorded in a `login_oauth_states` row keyed by the CSRF state, not
/// carried in cookies: cookies are per-site, so a sibling subdomain could set
/// values the callback would trust. The state itself is echoed in a host-only
/// cookie, which binds the callback to the browser that started the flow.
///
/// Accepts an optional `?invite_code=` query param, used for new user
/// registration. `?platform=ios` makes the callback redirect to the
/// `ownpulse://` scheme. `?mode=link` links Google to the already
/// authenticated user (resolved here, at initiation) instead of
/// logging in / registering.
pub async fn google_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(login_query): Query<GoogleLoginQuery>,
) -> Result<Response, ApiError> {
    let client_id = state
        .config
        .google_client_id
        .as_deref()
        .ok_or_else(|| ApiError::Internal("GOOGLE_CLIENT_ID not configured".to_string()))?;
    let redirect_uri = state.config.google_redirect_uri();

    let is_native = match login_query.platform.as_deref() {
        None | Some("web") => false,
        Some("ios") => true,
        Some(other) => {
            return Err(ApiError::BadRequest(format!("unknown platform: {other}")));
        }
    };

    // A near-miss (`Link`, `link `, an encoded variant) must not quietly mean
    // login: the user believes they are attaching Google to the account they
    // are signed into, and a silent fallthrough signs them in as whoever that
    // Google identity maps to instead.
    let is_link_mode = match login_query.mode.as_deref() {
        None => false,
        Some("link") => true,
        Some(other) => {
            return Err(ApiError::BadRequest(format!("unknown mode: {other}")));
        }
    };

    // Link mode resolves on the settings page, so every exit it can take is
    // a web URL. Pairing it with a native platform would end the flow on a
    // page inside an auth session that never completes.
    if is_link_mode && is_native {
        return Err(ApiError::BadRequest("link mode is web-only".to_string()));
    }

    // In link mode the user must already be authenticated, and the account
    // the Google identity will attach to is fixed here rather than at
    // callback time.
    let link_user_id = if is_link_mode {
        match extract_user_id_from_cookie(&headers, &state.config) {
            Some(user_id) => Some(user_id),
            None => {
                let redirect_url =
                    format!("{}/settings?error=auth_required", state.config.web_origin);
                return Ok(Redirect::to(&redirect_url).into_response());
            }
        }
    } else {
        None
    };

    // Unusable codes are dropped rather than rejected — the callback reports
    // a missing invite when registration actually needs one.
    let invite_code = login_query.invite_code.as_deref().filter(|code| {
        !code.is_empty() && code.len() <= 64 && code.chars().all(|c| c.is_alphanumeric())
    });

    let csrf_state = Uuid::new_v4();
    if let Err(e) = crate::db::login_oauth_states::insert(
        &state.pool,
        csrf_state,
        is_native,
        invite_code,
        link_user_id,
    )
    .await
    {
        // Never echo the invite code — not in the redirect, not in the log.
        tracing::error!(error = %e, "failed to store Google login OAuth state");
        // Send the caller back where it came from. A native caller sent to a
        // web page would sit in `ASWebAuthenticationSession` waiting for an
        // `ownpulse://` callback that can never arrive.
        let redirect_url = if is_native {
            "ownpulse://auth?error=server_error".to_string()
        } else if is_link_mode {
            format!("{}/settings?error=server_error", state.config.web_origin)
        } else {
            format!("{}/login?error=server_error", state.config.web_origin)
        };
        return Ok(Redirect::to(&redirect_url).into_response());
    }

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth\
         ?client_id={}\
         &redirect_uri={}\
         &response_type=code\
         &scope=openid%20email%20profile\
         &state={}",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        csrf_state,
    );

    let secure = secure_attr(&state.config.web_origin);
    // `Path=/` is what the `__Host-` prefix requires, and costs nothing here:
    // this is a ten-minute nonce, not a credential.
    let state_cookie = format!(
        "{}={csrf_state}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age=600",
        host_cookie_name(&state.config.web_origin, "oauth_state")
    );

    let mut response = Redirect::to(&auth_url).into_response();
    append_cookie(&mut response, &state_cookie)?;
    Ok(response)
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    /// Absent when the user declines consent, in which case Google sends
    /// `error` instead. Optional so that case reaches the handler and gets
    /// a redirect rather than failing extraction with a raw 400 body.
    pub code: Option<String>,
    /// CSRF state parameter — must match both the host-only `oauth_state`
    /// cookie and a live `login_oauth_states` row.
    pub state: Option<String>,
    /// Google's failure code, e.g. `access_denied` when consent is declined.
    pub error: Option<String>,
}

/// Where to send a browser whose Google registration failed the invite
/// check. `error` distinguishes an absent code from an unusable one so the
/// web app can say which; the code itself is never echoed back. A native
/// caller must get the custom scheme or its auth session never ends.
fn invite_error_redirect(is_native: bool, web_origin: &str, error: &str) -> String {
    if is_native {
        format!("ownpulse://auth?error={error}")
    } else {
        format!("{web_origin}/register?error={error}")
    }
}

/// GET /auth/google/callback?code=...&state=... — exchange authorization code,
/// find/create user, set httpOnly cookies or redirect to iOS.
pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<Response, ApiError> {
    // --- CSRF validation ---
    //
    // Both checks are required and neither substitutes for the other:
    //
    // 1. The host-only `oauth_state` cookie must match the `state` parameter.
    //    This binds the callback to the browser that started the flow —
    //    without it, an attacker who ran their own flow (this endpoint is
    //    unauthenticated) could navigate a victim to the callback with their
    //    own genuine state and land the victim in the attacker's account.
    //
    // 2. The state must resolve to a live `login_oauth_states` row, which is
    //    deleted on read. That row — not any cookie — is the only source for
    //    the platform, the invite code, and the link target.
    //
    // Failures here are a 400, not a redirect: the platform is only knowable
    // from the row, and redirecting an iOS user to a web page inside
    // `ASWebAuthenticationSession` hangs the session until they cancel.
    let cookie_state = read_cookie(
        &headers,
        &host_cookie_name(&state.config.web_origin, "oauth_state"),
    )
    .ok_or_else(|| ApiError::BadRequest("missing oauth_state cookie".into()))?;
    let query_state = query
        .state
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("missing state parameter".into()))?;
    if cookie_state != query_state {
        return Err(ApiError::BadRequest("OAuth state mismatch".into()));
    }
    let state_uuid =
        Uuid::parse_str(query_state).map_err(|_| ApiError::BadRequest("invalid state".into()))?;

    // Consume before the token exchange, not after: the delete is what makes
    // the state single-use, and two callbacks racing the same state must not
    // both get past this point. The cost is that a transient Google failure
    // burns the state and the user restarts the flow — the right trade.
    let login_state = crate::db::login_oauth_states::consume(&state.pool, state_uuid)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("invalid or expired OAuth state".into()))?;

    let is_native_app = login_state.is_native;
    let invite_code = login_state.invite_code;

    let secure = secure_attr(&state.config.web_origin);
    let clear_state_cookie = format!(
        "{}=; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age=0",
        host_cookie_name(&state.config.web_origin, "oauth_state")
    );

    // Declining consent is the most common unhappy path: Google returns
    // `error` and no `code`. The state is consumed above first, so this
    // still needs a valid flow to reach — and so the platform is known and
    // the caller gets a redirect it can follow rather than a raw body.
    let code = match query.code.as_deref() {
        Some(code) if query.error.is_none() => code,
        _ => {
            let reason = query.error.as_deref().unwrap_or("access_denied");
            tracing::info!(
                reason,
                "Google sign-in did not return an authorization code"
            );
            let redirect_url = if is_native_app {
                format!("ownpulse://auth?error={reason}")
            } else {
                format!("{}/login?error=google_declined", state.config.web_origin)
            };
            let mut response = Redirect::to(&redirect_url).into_response();
            append_cookie(&mut response, &clear_state_cookie)?;
            return Ok(response);
        }
    };

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
    let redirect_uri = state.config.google_redirect_uri();

    let tokens = crate::integrations::google::exchange_code_for_tokens(
        &state.http_client,
        client_id,
        client_secret,
        &redirect_uri,
        code,
        &state.config.google_token_url,
        // No PKCE on this flow — CSRF is the state cookie plus the row.
        None,
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

    // ---------------------------------------------------------------
    // Link mode: associate the Google account with an existing user.
    // ---------------------------------------------------------------
    // The link target was fixed at initiation, so no cookie present now can
    // redirect the Google identity to a different account.
    if let Some(linking_user_id) = login_state.link_user_id {
        // Verify user exists and is active.
        let linking_user = users::find_by_id(&state.pool, linking_user_id)
            .await
            .map_err(|_| ApiError::Forbidden)?;

        if linking_user.status != "active" {
            return Err(ApiError::Forbidden);
        }

        // Check if Google sub is already linked to a different user.
        match user_auth_methods::find_by_provider_subject(&state.pool, "google", &google_user.sub)
            .await
        {
            Ok(existing) if existing.id != linking_user_id => {
                let redirect_url =
                    format!("{}/settings?error=already_linked", state.config.web_origin);
                let mut response = Redirect::to(&redirect_url).into_response();
                append_cookie(&mut response, &clear_state_cookie)?;
                return Ok(response);
            }
            Ok(_) => {
                // Already linked to the same user — idempotent success.
            }
            Err(sqlx::Error::RowNotFound) => {
                user_auth_methods::insert(
                    &state.pool,
                    linking_user_id,
                    "google",
                    Some(&google_user.sub),
                    Some(&google_user.email),
                )
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            }
            Err(e) => return Err(ApiError::Internal(e.to_string())),
        }

        let redirect_url = format!("{}/settings?linked=google", state.config.web_origin);
        let mut response = Redirect::to(&redirect_url).into_response();
        append_cookie(&mut response, &clear_state_cookie)?;
        return Ok(response);
    }

    // ---------------------------------------------------------------
    // Login / register flow (existing behaviour).
    // ---------------------------------------------------------------
    let display_name = sanitize_username(google_user.email.split('@').next().unwrap_or("user"));

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

    let (user, google_is_first_user) = match existing_user {
        Ok(user) => {
            // Existing user — no invite needed.
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            (user, false)
        }
        Err(sqlx::Error::RowNotFound) => {
            // New user — before creating, check for email collision with an
            // existing account (e.g. a local user who registered with the same
            // email). This must happen inside the transaction to avoid TOCTOU.
            if users::email_exists_tx(&mut tx, &google_user.email)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
            {
                // Roll back — the invite (if any) was not yet claimed.
                tx.rollback()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;

                if is_native_app {
                    let redirect_url = "ownpulse://auth?error=email_exists";
                    let mut response = Redirect::to(redirect_url).into_response();
                    append_cookie(&mut response, &clear_state_cookie)?;
                    return Ok(response);
                } else {
                    let redirect_url =
                        format!("{}/login?error=email_exists", state.config.web_origin);
                    let mut response = Redirect::to(&redirect_url).into_response();
                    append_cookie(&mut response, &clear_state_cookie)?;
                    return Ok(response);
                }
            }

            // Skip invite requirement when this is the very first user (bootstrap).
            let is_first_user = users::is_empty_tx(&mut tx)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            // If first user, acquire advisory lock and re-check to prevent TOCTOU race
            let is_first_user = if is_first_user {
                users::acquire_bootstrap_lock_tx(&mut tx)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                // Re-check after acquiring lock — another request may have created a user
                users::is_empty_tx(&mut tx)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            } else {
                false
            };

            if is_first_user {
                tracing::info!("first user registration — invite requirement bypassed");
            }

            // Claim invite if required, then create.
            let claimed_invite = if state.config.require_invite && !is_first_user {
                let code = match invite_code {
                    Some(c) => c,
                    None => {
                        tx.rollback()
                            .await
                            .map_err(|e| ApiError::Internal(e.to_string()))?;

                        let redirect_url = invite_error_redirect(
                            is_native_app,
                            &state.config.web_origin,
                            "invite_required",
                        );
                        let mut response = Redirect::to(&redirect_url).into_response();
                        append_cookie(&mut response, &clear_state_cookie)?;
                        return Ok(response);
                    }
                };

                let invite = match invites::claim_invite_code_tx(&mut tx, &code).await {
                    Ok(invite) => invite,
                    Err(sqlx::Error::RowNotFound) => {
                        // Every path out of this handler is a browser
                        // navigation, so a JSON 400 would be rendered as
                        // raw text (and, on iOS, inside an auth session
                        // that never completes). Redirect like the
                        // missing-code case; the code itself is never
                        // echoed back.
                        tx.rollback()
                            .await
                            .map_err(|e| ApiError::Internal(e.to_string()))?;
                        tracing::info!(
                            "Google registration rejected: invite code invalid, expired or exhausted"
                        );
                        let redirect_url = invite_error_redirect(
                            is_native_app,
                            &state.config.web_origin,
                            "invite_invalid",
                        );
                        let mut response = Redirect::to(&redirect_url).into_response();
                        append_cookie(&mut response, &clear_state_cookie)?;
                        return Ok(response);
                    }
                    Err(other) => return Err(ApiError::Internal(other.to_string())),
                };
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

            // Promote first user to admin so they can create invite codes
            if is_first_user {
                users::promote_to_admin_tx(&mut tx, user.id)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                tracing::info!(user_id = %user.id, "first user promoted to admin");
            }

            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            (user, is_first_user)
        }
        Err(e) => return Err(ApiError::Internal(e.to_string())),
    };

    // Use "admin" for token if first user was promoted, since the struct still has "user"
    let effective_role = if google_is_first_user {
        "admin"
    } else {
        &user.role
    };

    if user.status != "active" {
        // Disabled users get a short-lived access token only (no refresh
        // token, no refresh cookie), so they can still reach export and
        // self-delete before it expires — the same entitlement password
        // login grants.
        //
        // It rides in the URL fragment rather than a cookie: no route
        // authenticates from the access cookie (every extractor reads the
        // Authorization header), so a cookie would deliver nothing the
        // client can use. A fragment is never sent to a server and is
        // readable by the client that needs it.
        let access_token = encode_access_token(
            user.id,
            effective_role,
            &state.config.jwt_secret,
            &state.config.web_origin,
            state.config.jwt_expiry_seconds,
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        let mut response = if is_native_app {
            Redirect::to(&format!("ownpulse://auth#token={access_token}")).into_response()
        } else {
            Redirect::to(&format!(
                "{}/?auth=disabled#token={access_token}",
                state.config.web_origin
            ))
            .into_response()
        };
        append_cookie(&mut response, &clear_state_cookie)?;
        return Ok(response);
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
        effective_role,
        &state.config.jwt_secret,
        &state.config.web_origin,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if is_native_app {
        // Native app flow: redirect to the custom URI scheme with tokens in the
        // URL fragment so the app can extract them from the redirect.
        // The app stores these tokens in the Keychain, never in cookies.
        let redirect_url = format!(
            "ownpulse://auth#token={}&refresh_token={}",
            access_token, raw_token
        );
        let mut response = Redirect::to(&redirect_url).into_response();
        append_cookie(&mut response, &clear_state_cookie)?;
        Ok(response)
    } else {
        // Web flow: set tokens as httpOnly cookies and redirect without tokens in URL.
        let access_cookie = format!(
            "{}={access_token}; HttpOnly{secure}; SameSite=Lax; Path=/; Max-Age={}",
            host_cookie_name(&state.config.web_origin, "access_token"),
            state.config.jwt_expiry_seconds
        );
        let refresh_cookie = format!(
            "refresh_token={raw_token}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
            state.config.refresh_token_expiry_seconds
        );

        let redirect_url = format!("{}/?auth=success", state.config.web_origin);
        let mut response = Redirect::to(&redirect_url).into_response();

        for cookie_str in [&access_cookie, &refresh_cookie, &clear_state_cookie] {
            append_cookie(&mut response, cookie_str)?;
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

    let (user, apple_is_first_user) = match existing_user {
        Ok(user) => {
            // Existing user — no invite needed.
            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            (user, false)
        }
        Err(sqlx::Error::RowNotFound) => {
            // New user — before creating, check for email collision with an
            // existing account (e.g. a user who registered with the same email
            // via Google or local auth). This must happen inside the transaction.
            if users::email_exists_tx(&mut tx, email)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
            {
                tx.rollback()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                return Err(ApiError::Conflict(
                    "an account with this email already exists \
                     — sign in with your existing method, then link Apple from Settings"
                        .into(),
                ));
            }

            // Skip invite requirement when this is the very first user (bootstrap).
            let is_first_user = users::is_empty_tx(&mut tx)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            // If first user, acquire advisory lock and re-check to prevent TOCTOU race
            let is_first_user = if is_first_user {
                users::acquire_bootstrap_lock_tx(&mut tx)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                // Re-check after acquiring lock — another request may have created a user
                users::is_empty_tx(&mut tx)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
            } else {
                false
            };

            if is_first_user {
                tracing::info!("first user registration — invite requirement bypassed");
            }

            // Claim invite if required, then create.
            let claimed_invite = if state.config.require_invite && !is_first_user {
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

            // Promote first user to admin so they can create invite codes
            if is_first_user {
                users::promote_to_admin_tx(&mut tx, user.id)
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;
                tracing::info!(user_id = %user.id, "first user promoted to admin");
            }

            tx.commit()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            (user, is_first_user)
        }
        Err(e) => return Err(ApiError::Internal(e.to_string())),
    };

    // Use "admin" for token if first user was promoted, since the struct still has "user"
    let effective_role = if apple_is_first_user {
        "admin"
    } else {
        &user.role
    };

    if user.status != "active" {
        // Disabled users get a short-lived access token only (no refresh token,
        // no refresh cookie). This lets them reach export and self-delete routes
        // before the token expires — same behaviour as password login.
        return issue_access_token_only(&state, user.id, effective_role).await;
    }

    let is_web = body.platform == "web";
    issue_tokens_response(&state, user.id, effective_role, is_web).await
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

            if password.len() < 10 {
                return Err(ApiError::BadRequest(
                    "password must be at least 10 characters".into(),
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
                "Google linking requires OAuth redirect — navigate to /api/v1/auth/google/login?mode=link".into(),
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
        &state.config.web_origin,
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
        &state.config.web_origin,
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

    let secure = secure_attr(&state.config.web_origin);
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

/// Build the refresh-rotation response: fresh access token plus the given
/// refresh token in the cookie (and, for native clients, the body). Performs
/// no database writes — the caller persists the token.
fn rotation_response(
    state: &AppState,
    user: &crate::models::user::UserRow,
    raw_refresh: String,
    is_web: bool,
) -> Result<Response, ApiError> {
    let access_token = encode_access_token(
        user.id,
        &user.role,
        &state.config.jwt_secret,
        &state.config.web_origin,
        state.config.jwt_expiry_seconds,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let secure = secure_attr(&state.config.web_origin);
    let cookie = format!(
        "refresh_token={raw_refresh}; HttpOnly{secure}; SameSite=Lax; Path=/api/v1/auth; Max-Age={}",
        state.config.refresh_token_expiry_seconds
    );

    let mut response = if is_web {
        let token_response = TokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: state.config.jwt_expiry_seconds,
        };
        (StatusCode::OK, Json(token_response)).into_response()
    } else {
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
        &state.config.web_origin,
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

    let secure = secure_attr(&state.config.web_origin);
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

/// POST /auth/forgot-password — request a password reset link.
///
/// Always returns 200 to prevent email enumeration.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    // Validate email format (basic check)
    if body.email.len() > 254 || !body.email.contains('@') {
        return Ok(StatusCode::OK);
    }

    // Look up user
    let user = match users::find_by_email(&state.pool, &body.email).await {
        Ok(u) => u,
        Err(_) => return Ok(StatusCode::OK),
    };

    // Disabled users get nothing
    if user.status != "active" {
        return Ok(StatusCode::OK);
    }

    // OAuth-only users (no password) get a helpful notice instead of a reset link
    if user.password_hash.is_none() {
        let provider = match user.auth_provider.as_str() {
            "google" => "Google",
            "apple" => "Apple",
            other => other,
        };
        let html_body = format!(
            "<p>Someone requested a password reset for your OwnPulse account.</p>\
             <p>Your account uses <strong>{provider}</strong> sign-in, so there is no password to reset. \
             Just use the \"{provider}\" button on the login page.</p>\
             <p>If you did not request this, you can safely ignore this email.</p>"
        );
        if let Err(e) = crate::email::send_email(
            &state.config,
            &user.email,
            "OwnPulse password reset request",
            &html_body,
        )
        .await
        {
            tracing::error!(error = %e, "failed to send OAuth notice email");
        }
        return Ok(StatusCode::OK);
    }

    // Generate token
    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_refresh_token(&raw_token, &state.config.jwt_secret);

    // Invalidate previous tokens for this user
    password_reset_tokens::invalidate_all_for_user(&state.pool, user.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Insert new token (expires in 1 hour)
    let expires_at = Utc::now() + chrono::Duration::hours(1);
    password_reset_tokens::insert(&state.pool, user.id, &token_hash, expires_at)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Build reset URL and send email
    let reset_url = format!(
        "{}/reset-password?token={}",
        state.config.web_origin, raw_token
    );
    let html_body = format!(
        "<p>You requested a password reset for your OwnPulse account.</p>\
         <p><a href=\"{reset_url}\">Reset your password</a></p>\
         <p>Or copy this link: {reset_url}</p>\
         <p>This link expires in 1 hour. If you did not request this, you can ignore this email.</p>"
    );

    if let Err(e) = crate::email::send_email(
        &state.config,
        &body.email,
        "Reset your OwnPulse password",
        &html_body,
    )
    .await
    {
        tracing::error!(error = %e, "failed to send password reset email");
    }

    Ok(StatusCode::OK)
}

/// POST /auth/reset-password — validate token and set new password.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    // Validate password length
    if body.password.len() < 10 {
        return Err(ApiError::BadRequest(
            "password must be at least 10 characters".into(),
        ));
    }

    // Hash the incoming token and look it up
    let token_hash = hash_refresh_token(&body.token, &state.config.jwt_secret);
    let token_row = password_reset_tokens::find_valid_by_hash(&state.pool, &token_hash)
        .await
        .map_err(|_| ApiError::BadRequest("invalid or expired reset token".into()))?;

    // Hash new password (before transaction — bcrypt is slow)
    let new_password_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Begin transaction
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Mark token as claimed
    password_reset_tokens::mark_claimed_tx(&mut tx, token_row.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Update password
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(token_row.user_id)
        .bind(&new_password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Revoke all refresh tokens (log out all sessions)
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(token_row.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(StatusCode::OK)
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

    /// The `__Host-` prefix and the `Secure` attribute must appear together:
    /// a browser drops a prefixed cookie sent without `Secure`, and a bare
    /// name sent with `Secure` silently loses the host-only guarantee. Both
    /// derive from the same predicate, and these pin them to it.
    #[test]
    fn host_cookie_name_and_secure_attr_agree_on_https() {
        for origin in [
            "https://app.ownpulse.health",
            "https://localhost:5173",
            "https://app.ownpulse.health:8443",
        ] {
            assert_eq!(secure_attr(origin), "; Secure", "origin: {origin}");
            assert_eq!(
                host_cookie_name(origin, "access_token"),
                "__Host-access_token",
                "origin: {origin}"
            );
            assert_eq!(
                host_cookie_name(origin, "oauth_state"),
                "__Host-oauth_state",
                "origin: {origin}"
            );
        }
    }

    #[test]
    fn host_cookie_name_and_secure_attr_agree_on_plain_http() {
        for origin in ["http://localhost:5173", "http://192.168.1.10:8080"] {
            assert_eq!(secure_attr(origin), "", "origin: {origin}");
            assert_eq!(
                host_cookie_name(origin, "access_token"),
                "access_token",
                "origin: {origin}"
            );
            assert_eq!(
                host_cookie_name(origin, "oauth_state"),
                "oauth_state",
                "origin: {origin}"
            );
        }
    }

    /// `https` has to be the scheme, not merely present in the origin.
    #[test]
    fn secure_attr_requires_the_https_scheme() {
        assert_eq!(secure_attr("http://https.example.com"), "");
        assert_eq!(host_cookie_name("http://https.example.com", "x"), "x");
    }

    #[test]
    fn invite_error_redirect_routes_native_callers_to_the_custom_scheme() {
        assert_eq!(
            invite_error_redirect(true, "https://app.ownpulse.health", "invite_required"),
            "ownpulse://auth?error=invite_required"
        );
        assert_eq!(
            invite_error_redirect(false, "https://app.ownpulse.health", "invite_required"),
            "https://app.ownpulse.health/register?error=invite_required"
        );
    }

    #[test]
    fn invite_error_redirect_distinguishes_an_unusable_code() {
        assert_eq!(
            invite_error_redirect(false, "https://app.ownpulse.health", "invite_invalid"),
            "https://app.ownpulse.health/register?error=invite_invalid"
        );
    }
}
