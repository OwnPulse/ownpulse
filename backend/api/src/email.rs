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

    let mut transport_builder =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)?
            .port(config.smtp_port);

    if let (Some(username), Some(password)) = (
        config.smtp_username.as_deref(),
        config.smtp_password.as_deref(),
    ) {
        transport_builder =
            transport_builder.credentials(Credentials::new(username.to_string(), password.to_string()));
    }

    let mailer = transport_builder.build();
    mailer.send(email).await?;

    Ok(())
}
