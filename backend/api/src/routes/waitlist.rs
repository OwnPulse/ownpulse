// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::error::ApiError;
use crate::AppState;

#[derive(Deserialize)]
pub struct WaitlistSignup {
    pub email: String,
}

pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<WaitlistSignup>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("INSERT INTO waitlist (email) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&body.email)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
