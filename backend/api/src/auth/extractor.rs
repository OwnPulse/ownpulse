// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::db::users;
use crate::error::ApiError;

use super::jwt::decode_access_token;

/// Decode a bearer token (from wherever the caller pulled it — header,
/// query param, or a cookie carrying the token across an OAuth redirect)
/// and load the active user's id/role. Factored out so header auth
/// ([`extract_active_user`]), query-param auth ([`AuthUserOrQueryToken`]),
/// and the Google Calendar OAuth callback's cookie-bound auth
/// (`routes::google_calendar::google_calendar_callback`) apply identical
/// validation.
pub(crate) async fn decode_and_load_active_user(
    token: &str,
    state: &AppState,
) -> Result<(Uuid, String), ApiError> {
    let claims = decode_access_token(token, &state.config.jwt_secret, &state.config.web_origin)
        .map_err(|_| ApiError::Unauthorized)?;

    // Check user status in the database — disabled/deleted users are rejected immediately
    let user = users::find_by_id(&state.pool, claims.sub)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    if user.status != "active" {
        return Err(ApiError::Forbidden);
    }

    Ok((user.id, user.role))
}

/// Shared helper: decode JWT, query the DB to verify the user exists and is active.
async fn extract_active_user(
    parts: &mut Parts,
    state: &AppState,
) -> Result<(Uuid, String), ApiError> {
    let header = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::Unauthorized)?;

    decode_and_load_active_user(token, state).await
}

/// Axum extractor that validates the `Authorization: Bearer <token>` header
/// and yields the authenticated user's ID and role.
pub struct AuthUser {
    pub id: Uuid,
    pub role: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let (id, role) = extract_active_user(parts, state).await?;
            Ok(AuthUser { id, role })
        })
    }
}

#[derive(serde::Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// Axum extractor for routes reached by plain browser navigation (an
/// `<a href>`, not a `fetch()` call) rather than an API client, which cannot
/// attach an `Authorization` header. Accepts the header when present,
/// otherwise falls back to a `?token=<JWT>` query parameter — the same
/// pattern `routes::events` uses for `EventSource`, which has the identical
/// no-custom-headers limitation. That fallback means the token can land in
/// server access logs; callers should only use it where a redirect-based
/// flow leaves no alternative, and should prefer the short-lived access
/// token (never a refresh token) to bound the exposure window.
///
/// Exposes the raw token string (not just the decoded identity) because the
/// Google Calendar connect flow forwards it one hop further, in a cookie, to
/// authenticate the OAuth callback — see `routes::google_calendar`.
pub struct AuthUserOrQueryToken {
    pub id: Uuid,
    pub role: String,
    pub token: String,
}

impl FromRequestParts<AppState> for AuthUserOrQueryToken {
    type Rejection = ApiError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let header_token = parts
                .headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);

            let token = match header_token {
                // Header present: it must be a well-formed bearer token —
                // don't silently mask a malformed header by falling back to
                // the query param.
                Some(header) => header
                    .strip_prefix("Bearer ")
                    .ok_or(ApiError::Unauthorized)?
                    .to_string(),
                None => {
                    let axum::extract::Query(query) =
                        axum::extract::Query::<TokenQuery>::from_request_parts(parts, state)
                            .await
                            .map_err(|_| ApiError::Unauthorized)?;
                    query.token.ok_or(ApiError::Unauthorized)?
                }
            };

            let (id, role) = decode_and_load_active_user(&token, state).await?;
            Ok(AuthUserOrQueryToken { id, role, token })
        })
    }
}

/// Shared helper: decode JWT, query the DB to verify the user exists.
/// Unlike [`extract_active_user`], this does NOT check whether the user is active.
/// Used for endpoints that disabled users must still access (export, self-delete).
async fn extract_any_user(parts: &mut Parts, state: &AppState) -> Result<(Uuid, String), ApiError> {
    let header = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::Unauthorized)?;

    let claims = decode_access_token(token, &state.config.jwt_secret, &state.config.web_origin)
        .map_err(|_| ApiError::Unauthorized)?;

    let user = users::find_by_id(&state.pool, claims.sub)
        .await
        .map_err(|_| ApiError::Unauthorized)?;

    Ok((user.id, user.role))
}

/// Axum extractor that validates the JWT and confirms the user exists, but does
/// NOT reject disabled users. Used for export and account deletion endpoints
/// where disabled users must retain access to their own data.
pub struct AuthUserAllowDisabled {
    pub id: Uuid,
    pub role: String,
}

impl FromRequestParts<AppState> for AuthUserAllowDisabled {
    type Rejection = ApiError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let (id, role) = extract_any_user(parts, state).await?;
            Ok(AuthUserAllowDisabled { id, role })
        })
    }
}

/// Axum extractor that validates the user is an admin.
pub struct AdminUser(pub Uuid);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Self, Self::Rejection>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let (id, role) = extract_active_user(parts, state).await?;
            if role != "admin" {
                return Err(ApiError::Forbidden);
            }
            Ok(AdminUser(id))
        })
    }
}
