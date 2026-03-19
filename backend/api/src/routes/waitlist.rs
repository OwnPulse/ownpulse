// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};

#[derive(Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct WaitlistResponse {
    pub ok: bool,
}

pub async fn signup(
    State(pool): State<PgPool>,
    Json(body): Json<WaitlistRequest>,
) -> (StatusCode, Json<WaitlistResponse>) {
    let email = body.email.trim().to_lowercase();

    if email.is_empty() || !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(WaitlistResponse { ok: false }),
        );
    }

    let result = sqlx::query(
        "INSERT INTO waitlist (email, name) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING",
    )
    .bind(&email)
    .bind(&body.name)
    .execute(&pool)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                info!(email = %email, "new waitlist signup");
            } else {
                info!(email = %email, "waitlist signup already exists");
            }
        }
        Err(e) => {
            warn!(error = %e, "failed to insert waitlist signup");
        }
    }

    // Always return ok — don't leak whether the email exists
    (StatusCode::OK, Json(WaitlistResponse { ok: true }))
}
