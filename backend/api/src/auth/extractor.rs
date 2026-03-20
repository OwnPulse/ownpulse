// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::error::ApiError;
use crate::AppState;

use super::jwt::decode_access_token;

/// Axum extractor that validates the `Authorization: Bearer <token>` header
/// and yields the authenticated user's ID.
pub struct AuthUser(pub Uuid);

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
            let header = parts
                .headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(ApiError::Unauthorized)?;

            let token = header
                .strip_prefix("Bearer ")
                .ok_or(ApiError::Unauthorized)?;

            let claims =
                decode_access_token(token, &state.config.jwt_secret).map_err(|_| ApiError::Unauthorized)?;

            Ok(AuthUser(claims.sub))
        })
    }
}
