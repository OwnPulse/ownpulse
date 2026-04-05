// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use crate::AppState;
use crate::db::feature_flags;
use crate::error::ApiError;
use crate::models::feature_flag::{ConfigResponse, IosConfig};

/// GET /api/v1/config — public, unauthenticated endpoint returning feature
/// flags and iOS configuration. Intended for clients to check capabilities
/// before (or without) authenticating.
pub async fn get_config(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let flags = feature_flags::all_flags(&state.pool).await?;

    let response = ConfigResponse {
        feature_flags: flags,
        ios: IosConfig {
            min_supported_version: state.config.ios_min_version.clone(),
            force_upgrade_below: state.config.ios_force_upgrade_below.clone(),
        },
    };

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(response),
    ))
}
