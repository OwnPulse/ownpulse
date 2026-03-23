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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
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
        };

        (status, Json(json!({"error": message}))).into_response()
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
