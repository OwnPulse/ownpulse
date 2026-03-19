// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// Holds the Axum app, database pool, and the container handle (which keeps
/// the ephemeral Postgres alive for the lifetime of the test).
pub struct TestApp {
    pub app: Router,
    pub pool: PgPool,
    // The container is kept alive by holding this handle; dropping it stops Postgres.
    pub _container: testcontainers::ContainerAsync<Postgres>,
}

/// Build a test-friendly config with defaults suitable for integration tests.
fn test_config(database_url: &str) -> api::config::Config {
    api::config::Config {
        database_url: database_url.to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        garmin_client_id: None,
        garmin_client_secret: None,
        oura_client_id: None,
        oura_client_secret: None,
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
    }
}

/// Spin up an ephemeral Postgres via testcontainers, run all migrations, and
/// return a ready-to-use [`TestApp`].
pub async fn setup() -> TestApp {
    let container = Postgres::default()
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
    let state = api::AppState {
        pool: pool.clone(),
        config,
        http_client: reqwest::Client::new(),
    };

    let app = api::build_app(state);

    TestApp {
        app,
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
        .unwrap_or_else(|e| panic!("cannot read migrations dir {}: {e}", migrations_dir.display()))
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
        sqlx::query(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migration {} failed: {e}", path.display()));
    }
}
