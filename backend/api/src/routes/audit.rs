// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::State;
use axum::Json;

use crate::auth::extractor::AuthUser;
use crate::db::audit::{self, AuditEntry};
use crate::error::ApiError;
use crate::AppState;

/// GET /account/audit-log — return the caller's last 100 audit log entries.
pub async fn list_audit_log(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<AuditEntry>>, ApiError> {
    let entries = audit::list_for_user(&state.pool, user_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(entries))
}
