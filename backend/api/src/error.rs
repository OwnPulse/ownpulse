// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    NotFoundMsg(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("database schema outdated — migrations may need to be run")]
    SchemaOutdated,
    #[error("{0}")]
    NotImplemented(String),
    #[error("upstream error: {0}")]
    BadGateway(String),
    #[error("rate limited, retry after {retry_after_secs}s")]
    TooManyRequests { retry_after_secs: u64 },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // `TooManyRequests` needs an extra `Retry-After` header, so it's built
        // separately rather than folded into the shared (status, message) match.
        if let ApiError::TooManyRequests { retry_after_secs } = &self {
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error": self.to_string()})),
            )
                .into_response();
            if let Ok(value) = retry_after_secs.to_string().parse() {
                response.headers_mut().insert("retry-after", value);
            }
            return response;
        }

        let (status, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::NotFoundMsg(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            ApiError::SchemaOutdated => (
                StatusCode::SERVICE_UNAVAILABLE,
                "database schema outdated — migrations may need to be run".to_string(),
            ),
            ApiError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg.clone()),
            ApiError::BadGateway(msg) => {
                tracing::warn!(error = %msg, "upstream integration error");
                (StatusCode::BAD_GATEWAY, msg.clone())
            }
            ApiError::TooManyRequests { .. } => unreachable!("handled above"),
        };

        (status, Json(json!({"error": message}))).into_response()
    }
}

impl From<crate::jobs::SyncError> for ApiError {
    fn from(err: crate::jobs::SyncError) -> Self {
        match err {
            crate::jobs::SyncError::NotConnected => {
                ApiError::NotFoundMsg("integration is not connected".to_string())
            }
            crate::jobs::SyncError::NotConfigured => ApiError::NotImplemented(
                "this integration is not configured on this server — the operator needs to set \
                 the corresponding client id/secret environment variables"
                    .to_string(),
            ),
            crate::jobs::SyncError::RateLimited { retry_after_secs } => {
                ApiError::TooManyRequests { retry_after_secs }
            }
            crate::jobs::SyncError::Upstream(msg) => {
                tracing::warn!(error = %msg, "sync failed");
                ApiError::BadGateway("sync failed — see server logs for details".to_string())
            }
        }
    }
}

impl From<crate::crypto::CryptoError> for ApiError {
    fn from(err: crate::crypto::CryptoError) -> Self {
        ApiError::Internal(format!("crypto error: {err}"))
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            sqlx::Error::Database(db_err) => {
                // 23505 = unique_violation
                if db_err.code().as_deref() == Some("23505") {
                    ApiError::Conflict("resource already exists".to_string())
                }
                // 42P01 = undefined_table ("relation does not exist")
                else if db_err.code().as_deref() == Some("42P01") {
                    tracing::error!(
                        error = %err,
                        "query referenced a missing table — database schema is likely outdated"
                    );
                    ApiError::SchemaOutdated
                } else {
                    ApiError::Internal(err.to_string())
                }
            }
            _ => ApiError::Internal(err.to_string()),
        }
    }
}
