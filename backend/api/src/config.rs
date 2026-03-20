// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde::Deserialize;

/// Application configuration loaded from environment variables via `envy`.
///
/// Required variables: DATABASE_URL, JWT_SECRET, ENCRYPTION_KEY, APP_USER, APP_PASSWORD_HASH, WEB_ORIGIN.
/// All integration credentials are optional — the server starts without them;
/// the corresponding sync jobs simply won't run.
#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub database_url: String,

    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_seconds: u64,
    #[serde(default = "default_refresh_expiry")]
    pub refresh_token_expiry_seconds: u64,

    #[serde(default)]
    pub google_client_id: Option<String>,
    #[serde(default)]
    pub google_client_secret: Option<String>,
    #[serde(default)]
    pub google_redirect_uri: Option<String>,

    #[serde(default = "default_google_token_url")]
    pub google_token_url: String,
    #[serde(default = "default_google_userinfo_url")]
    pub google_userinfo_url: String,

    #[serde(default)]
    pub garmin_client_id: Option<String>,
    #[serde(default)]
    pub garmin_client_secret: Option<String>,

    #[serde(default)]
    pub oura_client_id: Option<String>,
    #[serde(default)]
    pub oura_client_secret: Option<String>,

    #[serde(default)]
    pub dexcom_client_id: Option<String>,
    #[serde(default)]
    pub dexcom_client_secret: Option<String>,

    #[serde(default = "default_encryption_key")]
    pub encryption_key: String,

    #[serde(default)]
    pub storage_path: Option<String>,

    #[serde(default)]
    pub app_user: Option<String>,
    #[serde(default)]
    pub app_password_hash: Option<String>,

    #[serde(default = "default_data_region")]
    pub data_region: String,

    #[serde(default = "default_web_origin")]
    pub web_origin: String,

    #[serde(default = "default_rust_log")]
    pub rust_log: String,
}

fn default_jwt_secret() -> String {
    "dev-only-change-me".to_string()
}

fn default_jwt_expiry() -> u64 {
    3600
}

fn default_refresh_expiry() -> u64 {
    2_592_000
}

fn default_encryption_key() -> String {
    "0000000000000000000000000000000000000000000000000000000000000000".to_string()
}

fn default_data_region() -> String {
    "us".to_string()
}

fn default_web_origin() -> String {
    "http://localhost:5173".to_string()
}

fn default_google_token_url() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

fn default_google_userinfo_url() -> String {
    "https://www.googleapis.com/oauth2/v3/userinfo".to_string()
}

fn default_rust_log() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Self {
        envy::from_env::<Config>().expect("failed to load config from environment")
    }
}
