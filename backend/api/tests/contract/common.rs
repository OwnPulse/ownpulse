// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Shared setup for contract tests. Spins up testcontainers Postgres and
//! starts the Axum server on a random TCP port so the Pact verifier can
//! make real HTTP requests against it.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ImageExt;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// Holds the running server address, database pool, and the container handle.
pub struct ContractTestApp {
    pub port: u16,
    pub pool: PgPool,
    // Kept alive so the container isn't dropped.
    pub _container: testcontainers::ContainerAsync<Postgres>,
}

/// Build a test-friendly config.
fn test_config(database_url: &str) -> api::config::Config {
    api::config::Config {
        database_url: database_url.to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        google_token_url: "https://oauth2.googleapis.com/token".to_string(),
        google_userinfo_url: "https://www.googleapis.com/oauth2/v3/userinfo".to_string(),
        garmin_client_id: None,
        garmin_client_secret: None,
        garmin_base_url: None,
        oura_client_id: None,
        oura_client_secret: None,
        oura_api_base_url: None,
        oura_auth_base_url: None,
        dexcom_client_id: None,
        dexcom_client_secret: None,
        encryption_key: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        storage_path: None,
        app_user: None,
        app_password_hash: None,
        data_region: "us".to_string(),
        web_origin: "http://localhost:5173".to_string(),
        rust_log: "info".to_string(),
        encryption_key_previous: None,
        apple_client_id: None,
        apple_jwks_url: "https://appleid.apple.com/auth/keys".to_string(),
        require_invite: false,
        ios_min_version: None,
        ios_force_upgrade_below: None,
        smtp_host: None,
        smtp_port: 587,
        smtp_username: None,
        smtp_password: None,
        smtp_from: None,
    }
}

/// Spin up Postgres, run migrations, start the Axum server on a random port.
pub async fn setup() -> ContractTestApp {
    let container = Postgres::default()
        .with_tag("17-alpine")
        .start()
        .await
        .expect("failed to start postgres container");

    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("failed to get mapped port");

    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to testcontainers postgres");

    run_migrations(&pool).await;

    let config = test_config(&database_url);
    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: pool.clone(),
        config,
        http_client: reqwest::Client::new(),
        migrations_ready: Arc::new(AtomicBool::new(true)),
        event_tx,
    };

    let app = api::build_app_without_metrics(state);

    // Bind to port 0 to get a random available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    // Spawn the server in the background
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server error");
    });

    ContractTestApp {
        port,
        pool,
        _container: container,
    }
}

/// Read every SQL migration file from `db/migrations/` and execute them in
/// filename order.
async fn run_migrations(pool: &PgPool) {
    let migrations_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../db/migrations");

    let mut entries: Vec<_> = std::fs::read_dir(&migrations_dir)
        .unwrap_or_else(|e| {
            panic!(
                "cannot read migrations dir {}: {e}",
                migrations_dir.display()
            )
        })
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sql") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    entries.sort();

    for path in entries {
        let sql = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migration {} failed: {e}", path.display()));
    }
}
