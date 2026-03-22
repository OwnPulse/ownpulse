// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::time::Duration;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    match std::env::args().nth(1).as_deref() {
        Some("--health-check") => health_check().await,
        Some("--migrate-only") => migrate_only().await,
        Some(unknown) => {
            anyhow::bail!(
                "unknown argument: {unknown}. Use --health-check, --migrate-only, or no arguments to start the server"
            );
        }
        None => run_server().await,
    }
}

fn init_tracing() {
    if std::env::var("RUST_LOG")
        .unwrap_or_default()
        .contains("pretty")
    {
        tracing_subscriber::fmt().pretty().init();
    } else {
        tracing_subscriber::fmt().json().init();
    }
}

async fn health_check() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
        .context("failed to connect to database")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("health check query failed")?;

    info!("health check passed");
    Ok(())
}

async fn migrate_only() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .context("failed to connect to database")?;

    api::migrate::run_migrations(&pool).await?;

    info!("migrations complete");
    Ok(())
}

async fn run_server() -> anyhow::Result<()> {
    let config = api::config::Config::load();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let state = api::AppState {
        pool,
        config,
        http_client: reqwest::Client::new(),
    };

    let app = api::build_app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .context("failed to bind to port 8080")?;

    info!("listening on 0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received");
}
