// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use crate::AppState;

#[derive(Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
    pub name: Option<String>,
}

pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<WaitlistRequest>,
) -> impl IntoResponse {
    let email = body.email.trim().to_lowercase();

    if email.is_empty() || !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false})),
        );
    }

    let result = sqlx::query(
        "INSERT INTO waitlist (email, name) VALUES ($1, $2) ON CONFLICT (email) DO NOTHING",
    )
    .bind(&email)
    .bind(&body.name)
    .execute(&state.pool)
    .await;

    // Log only the domain part of the email to avoid PII in logs.
    let domain = email.rsplit('@').next().unwrap_or("unknown");

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                info!(email_domain = %domain, "new waitlist signup");
            } else {
                info!(email_domain = %domain, "waitlist signup already exists");
            }
        }
        Err(e) => {
            warn!(error = %e, "failed to insert waitlist signup");
        }
    }

    // Always return ok — don't leak whether the email exists
    (StatusCode::OK, Json(json!({"ok": true})))
}
