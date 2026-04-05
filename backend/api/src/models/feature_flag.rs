// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Response body for `GET /api/v1/config`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub feature_flags: HashMap<String, bool>,
    pub ios: IosConfig,
}

/// iOS-specific configuration returned as part of `/api/v1/config`.
#[derive(Debug, Serialize, Deserialize)]
pub struct IosConfig {
    pub min_supported_version: Option<String>,
    pub force_upgrade_below: Option<String>,
}

/// Request body for `PUT /api/v1/admin/feature-flags/:key`.
#[derive(Debug, Deserialize)]
pub struct UpsertFlagRequest {
    pub enabled: bool,
    pub description: Option<String>,
}

/// Full feature flag row returned by admin list endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct FeatureFlagResponse {
    pub id: uuid::Uuid,
    pub key: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
