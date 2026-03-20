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

const VALID_PERSONAS: &[&str] = &[
    "quantified_self", "biohacker", "peptide_pioneer", "iron_scientist",
    "health_detective", "builder", "clinician", "basics",
];

#[derive(Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
    pub name: Option<String>,
    pub persona: Option<String>,
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

    if let Some(ref p) = body.persona {
        if !VALID_PERSONAS.contains(&p.as_str()) {
            return (StatusCode::BAD_REQUEST, Json(json!({"ok": false})));
        }
    }

    let result = sqlx::query(
        "INSERT INTO waitlist (email, name, persona) VALUES ($1, $2, $3) ON CONFLICT (email) DO NOTHING",
    )
    .bind(&email)
    .bind(&body.name)
    .bind(&body.persona)
    .execute(&state.pool)
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
    (StatusCode::OK, Json(json!({"ok": true})))
}
