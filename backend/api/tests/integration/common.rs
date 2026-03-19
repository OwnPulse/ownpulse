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

    let app = api::build_app(pool.clone());

    TestApp {
        app,
        pool,
        _container: container,
    }
}

/// Read every SQL migration file from `db/migrations/` and execute them in
/// filename order. This mirrors what `sqlx migrate run` does, without requiring
/// the CLI.
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
