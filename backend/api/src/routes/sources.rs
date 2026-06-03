// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Source overlap discovery.
//!
//! Helps the user resolve which source should be the source of truth when more
//! than one connected device records the same metric. Writing the chosen
//! preference reuses the existing `/source-preferences` write path — this
//! module only handles discovery.

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::source_preferences as db;
use crate::error::ApiError;
use crate::models::source_preference::OverlapScanResponse;

/// GET /sources/overlap-scan
///
/// Scans the last 30 days for metrics that have overlapping records from more
/// than one source, returning per-metric the competing sources (ordered by
/// descending record count). Used by the iOS source-preference wizard to ask
/// the user which source should win for each contested metric.
pub async fn overlap_scan(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<OverlapScanResponse>, ApiError> {
    let metrics = db::overlap_scan(&state.pool, user_id).await?;
    Ok(Json(OverlapScanResponse { metrics }))
}
