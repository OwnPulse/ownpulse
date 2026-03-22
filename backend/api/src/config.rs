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

    /// Apple Sign-In client ID (also called "Service ID" for web or the app bundle ID for iOS).
    #[serde(default)]
    pub apple_client_id: Option<String>,
    /// Apple JWKS endpoint — overridable for tests.
    #[serde(default = "default_apple_jwks_url")]
    pub apple_jwks_url: String,

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
    /// Previous encryption key, used as fallback when decrypting legacy
    /// (unversioned) values during key rotation. Unset once all values have
    /// been re-encrypted with the current key.
    #[serde(default)]
    pub encryption_key_previous: Option<String>,

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

pub fn default_apple_jwks_url() -> String {
    "https://appleid.apple.com/auth/keys".to_string()
}

fn default_rust_log() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Self {
        let config =
            envy::from_env::<Config>().expect("failed to load config from environment");
        config.validate();
        config
    }

    /// Panic if insecure defaults are used outside localhost development.
    ///
    /// This prevents accidentally running production with the placeholder
    /// `JWT_SECRET` or an all-zero `ENCRYPTION_KEY`.
    fn validate(&self) {
        let is_localhost = self.web_origin.starts_with("http://localhost");

        if !is_localhost && self.jwt_secret == "dev-only-change-me" {
            panic!(
                "JWT_SECRET is still the default placeholder — \
                 set a real secret before running in production"
            );
        }

        if !is_localhost && self.encryption_key.chars().all(|c| c == '0') {
            panic!(
                "ENCRYPTION_KEY is all zeros — \
                 set a real 32-byte hex key before running in production"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `Config` with safe test defaults. Individual tests override
    /// the fields they care about.
    fn test_config() -> Config {
        Config {
            database_url: "postgres://localhost/test".to_string(),
            jwt_secret: default_jwt_secret(),
            jwt_expiry_seconds: default_jwt_expiry(),
            refresh_token_expiry_seconds: default_refresh_expiry(),
            google_client_id: None,
            google_client_secret: None,
            google_redirect_uri: None,
            garmin_client_id: None,
            garmin_client_secret: None,
            oura_client_id: None,
            oura_client_secret: None,
            dexcom_client_id: None,
            dexcom_client_secret: None,
            encryption_key: default_encryption_key(),
            encryption_key_previous: None,
            google_token_url: default_google_token_url(),
            google_userinfo_url: default_google_userinfo_url(),
            apple_client_id: None,
            apple_jwks_url: default_apple_jwks_url(),
            storage_path: None,
            app_user: None,
            app_password_hash: None,
            data_region: default_data_region(),
            web_origin: default_web_origin(),
            rust_log: default_rust_log(),
        }
    }

    #[test]
    fn default_config_with_localhost_does_not_panic() {
        let config = test_config();
        config.validate(); // should not panic
    }

    #[test]
    #[should_panic(expected = "JWT_SECRET")]
    fn default_jwt_secret_panics_in_production() {
        let mut config = test_config();
        config.web_origin = "https://app.ownpulse.health".to_string();
        config.validate();
    }

    #[test]
    #[should_panic(expected = "ENCRYPTION_KEY")]
    fn all_zero_encryption_key_panics_in_production() {
        let mut config = test_config();
        config.web_origin = "https://app.ownpulse.health".to_string();
        config.jwt_secret = "a-real-secret-that-is-not-the-default".to_string();
        config.validate();
    }
}
