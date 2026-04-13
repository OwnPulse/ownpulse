// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::config::Config;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tracing::warn;

/// Send an email via SMTP. If SMTP is not configured, logs a warning and returns Ok(()).
pub async fn send_email(
    config: &Config,
    to: &str,
    subject: &str,
    html_body: &str,
) -> Result<(), anyhow::Error> {
    let smtp_host = match config.smtp_host.as_deref() {
        Some(host) => host,
        None => {
            warn!("SMTP not configured (SMTP_HOST unset), skipping email to {to}");
            return Ok(());
        }
    };

    let from = config
        .smtp_from
        .as_deref()
        .unwrap_or("noreply@ownpulse.health");

    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html_body.to_string())?;

    // Implicit TLS on ports 465/2465; STARTTLS on ports 587/2587/other
    let mut transport_builder = if config.smtp_port == 465 || config.smtp_port == 2465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?.port(config.smtp_port)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)?.port(config.smtp_port)
    };

    if let (Some(username), Some(password)) = (
        config.smtp_username.as_deref(),
        config.smtp_password.as_deref(),
    ) {
        transport_builder = transport_builder
            .credentials(Credentials::new(username.to_string(), password.to_string()));
    }

    let mailer = transport_builder.build();
    mailer.send(email).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Config for unit testing.
    fn test_config() -> Config {
        Config {
            database_url: String::new(),
            jwt_secret: "test-secret-at-least-32-bytes-long!!".to_string(),
            jwt_expiry_seconds: 3600,
            refresh_token_expiry_seconds: 2_592_000,
            google_client_id: None,
            google_client_secret: None,
            google_redirect_uri: None,
            google_token_url: "https://oauth2.googleapis.com/token".to_string(),
            google_userinfo_url: "https://www.googleapis.com/oauth2/v3/userinfo".to_string(),
            apple_client_id: None,
            apple_jwks_url: "https://appleid.apple.com/auth/keys".to_string(),
            garmin_client_id: None,
            garmin_client_secret: None,
            garmin_base_url: None,
            oura_client_id: None,
            oura_client_secret: None,
            oura_api_base_url: None,
            oura_auth_base_url: None,
            dexcom_client_id: None,
            dexcom_client_secret: None,
            encryption_key: "0".repeat(64),
            encryption_key_previous: None,
            storage_path: None,
            app_user: None,
            app_password_hash: None,
            data_region: "us".to_string(),
            web_origin: "http://localhost:5173".to_string(),
            rust_log: "info".to_string(),
            require_invite: false,
            ios_min_version: None,
            ios_force_upgrade_below: None,
            smtp_host: None,
            smtp_port: 2587,
            smtp_username: None,
            smtp_password: None,
            smtp_from: None,
        }
    }

    #[tokio::test]
    async fn test_send_email_smtp_not_configured_returns_ok() {
        let config = test_config();
        assert!(config.smtp_host.is_none());
        let result = send_email(&config, "test@example.com", "Subject", "<p>Body</p>").await;
        assert!(result.is_ok());
    }
}
