// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::Resource;
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Holds the OTel tracer provider so we can flush spans on shutdown.
static TRACER_PROVIDER: OnceLock<opentelemetry_sdk::trace::SdkTracerProvider> = OnceLock::new();

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
    use tracing_subscriber::fmt;

    let pretty = std::env::var("RUST_LOG")
        .unwrap_or_default()
        .contains("pretty");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Build the optional OTel layer. `Option<Layer>` implements `Layer`, so we
    // always compose it into the subscriber — when `None` it is a no-op.
    let otel_layer = build_otel_layer();

    // Use `Option` layers for the fmt variants so the subscriber has a single
    // concrete type regardless of `pretty`.
    let json_layer = if pretty {
        None
    } else {
        Some(fmt::layer().json())
    };
    let pretty_layer = if pretty {
        Some(fmt::layer().pretty())
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(json_layer)
        .with(pretty_layer)
        .with(otel_layer)
        .init();
}

/// Attempt to create an OpenTelemetry tracing layer from the
/// `OTEL_EXPORTER_OTLP_ENDPOINT` env var. Returns `None` when the var is
/// unset or the exporter fails to initialise — the server continues without
/// trace export in that case.
fn build_otel_layer<S>() -> Option<tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::SdkTracer>>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;

    let resource = Resource::builder()
        .with_service_name("ownpulse-api")
        .build();

    // Build the exporter, optionally with TLS if OTEL_EXPORTER_OTLP_CERTIFICATE
    // points to a CA cert (used for in-cluster TLS with cert-manager's internal CA).
    let exporter = build_otlp_exporter(&endpoint)?;

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer("ownpulse-api");

    // Store for graceful shutdown; ignore if already set (should not happen).
    let _ = TRACER_PROVIDER.set(provider);

    // Log after the subscriber is installed would be cleaner, but we cannot
    // emit tracing events before init. Use eprintln for this one message.
    eprintln!("OpenTelemetry trace export enabled → {endpoint}");

    Some(tracing_opentelemetry::layer().with_tracer(tracer))
}

/// Build an OTLP span exporter, optionally with TLS.
fn build_otlp_exporter(endpoint: &str) -> Option<opentelemetry_otlp::SpanExporter> {
    // Check for CA cert to enable TLS
    let ca_path = std::env::var("OTEL_EXPORTER_OTLP_CERTIFICATE").ok();

    if let Some(ref ca_path) = ca_path {
        // TLS path: build a tonic Channel with custom CA cert, pass it to the exporter
        let pem = match std::fs::read_to_string(ca_path) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("failed to read OTLP CA cert at {ca_path} ({err}), continuing without trace export");
                return None;
            }
        };

        let mut tls_config = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(pem));

        // mTLS: if client cert and key are available, present them to the server
        if let (Ok(cert_path), Ok(key_path)) = (
            std::env::var("OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE"),
            std::env::var("OTEL_EXPORTER_OTLP_CLIENT_KEY"),
        ) {
            match (std::fs::read_to_string(&cert_path), std::fs::read_to_string(&key_path)) {
                (Ok(cert_pem), Ok(key_pem)) => {
                    let identity = tonic::transport::Identity::from_pem(cert_pem, key_pem);
                    tls_config = tls_config.identity(identity);
                }
                (Err(err), _) | (_, Err(err)) => {
                    eprintln!("failed to read OTLP client cert/key ({err}), continuing without mTLS");
                }
            }
        }

        let channel = match tonic::transport::Channel::from_shared(endpoint.to_string()) {
            Ok(c) => match c.tls_config(tls_config) {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("failed to configure TLS ({err}), continuing without trace export");
                    return None;
                }
            },
            Err(err) => {
                eprintln!("invalid OTLP endpoint ({err}), continuing without trace export");
                return None;
            }
        };

        // Connect lazily — the channel will establish the connection on first use
        let channel = channel.connect_lazy();

        match opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_channel(channel)
            .build()
        {
            Ok(e) => Some(e),
            Err(err) => {
                eprintln!("failed to create OTLP exporter ({err}), continuing without trace export");
                None
            }
        }
    } else {
        // No TLS — plain gRPC connection
        match opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
        {
            Ok(e) => Some(e),
            Err(err) => {
                eprintln!("failed to create OTLP exporter ({err}), continuing without trace export");
                None
            }
        }
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

    // Check migration status. The server still starts even if migrations are
    // behind — the readiness probe (/readyz) will return 503, preventing
    // Kubernetes from routing traffic until the schema catches up.
    let migrations_ready = api::migration_check::new_migrations_ready();
    api::migration_check::run_check_and_set_flag(&pool, &migrations_ready).await;

    if !migrations_ready.load(std::sync::atomic::Ordering::SeqCst) {
        warn!(
            "server starting with outdated database schema — \
             /readyz will return 503 until migrations are applied"
        );
    }

    let state = api::AppState {
        pool,
        config,
        http_client: reqwest::Client::new(),
        migrations_ready,
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

    info!("shutdown signal received, flushing traces");
    if let Some(provider) = TRACER_PROVIDER.get()
        && let Err(err) = provider.shutdown()
    {
        warn!(error = %err, "failed to flush OpenTelemetry traces on shutdown");
    }
}
