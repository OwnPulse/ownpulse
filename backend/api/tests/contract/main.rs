// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Pact provider verification tests.
//!
//! Reads consumer contracts from `pact/contracts/*.json`, spins up the API
//! against a testcontainers Postgres, and replays every interaction to verify
//! the provider still satisfies them.

mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[allow(deprecated)]
use pact_verifier::{
    FilterInfo, NullRequestFilterExecutor, PactSource, ProviderInfo, ProviderTransport,
    VerificationOptions, callback_executors::ProviderStateExecutor, verify_provider_async,
};
use serde_json::Value;
use uuid::Uuid;

use common::ContractTestApp;

/// Provider state callback that sets up the database state required by each
/// Pact interaction.
#[derive(Debug)]
struct StateExecutor {
    pool: sqlx::PgPool,
    jwt_secret: String,
}

#[async_trait::async_trait]
impl ProviderStateExecutor for StateExecutor {
    async fn call(
        self: Arc<Self>,
        _interaction_id: Option<String>,
        provider_state: &pact_models::provider_states::ProviderState,
        _setup: bool,
        _client: Option<&reqwest::Client>,
    ) -> anyhow::Result<HashMap<String, Value>> {
        let state_name = &provider_state.name;
        let mut result = HashMap::new();

        match state_name.as_str() {
            "an authenticated user exists" => {
                let (user_id, token) = create_test_user(&self.pool, &self.jwt_secret).await;
                result.insert("user_id".to_string(), Value::String(user_id.to_string()));
                result.insert("token".to_string(), Value::String(token));
            }
            "an admin user exists" => {
                let (user_id, token) = create_admin_user(&self.pool, &self.jwt_secret).await;
                result.insert("user_id".to_string(), Value::String(user_id.to_string()));
                result.insert("token".to_string(), Value::String(token));
            }
            _ => {
                // Unknown state — log but don't fail; the interaction may not
                // need any particular setup.
                tracing::warn!("unhandled provider state: {state_name}");
            }
        }

        Ok(result)
    }

    fn teardown(&self) -> bool {
        false
    }
}

async fn create_test_user(pool: &sqlx::PgPool, jwt_secret: &str) -> (Uuid, String) {
    let hash = bcrypt::hash("testpassword", 4).expect("bcrypt hash");
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, password_hash, auth_provider) \
         VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(format!("contract-user-{}@example.com", Uuid::new_v4()))
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("insert test user");

    let token =
        api::auth::jwt::encode_access_token(row.0, "user", jwt_secret, 3600).expect("encode JWT");

    (row.0, token)
}

async fn create_admin_user(pool: &sqlx::PgPool, jwt_secret: &str) -> (Uuid, String) {
    let hash = bcrypt::hash("testpassword", 4).expect("bcrypt hash");
    let uid = Uuid::new_v4();
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, username, password_hash, auth_provider, role) \
         VALUES ($1, $2, $3, 'local', 'admin') RETURNING id",
    )
    .bind(format!("contract-admin-{}@example.com", uid))
    .bind(format!("admin-{}", &uid.to_string()[..8]))
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("insert admin user");

    let token =
        api::auth::jwt::encode_access_token(row.0, "admin", jwt_secret, 3600).expect("encode JWT");

    (row.0, token)
}

/// Resolve the path to `pact/contracts/` relative to the workspace root.
fn contracts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pact/contracts")
}

/// Verify a single contract file against the running provider.
async fn verify_contract(app: &ContractTestApp, contract_path: PathBuf) {
    let transport = ProviderTransport {
        transport: "HTTP".to_string(),
        port: Some(app.port),
        path: None,
        scheme: Some("http".to_string()),
    };

    #[allow(deprecated)]
    let provider = ProviderInfo {
        name: "ownpulse-api".to_string(),
        host: "127.0.0.1".to_string(),
        port: Some(app.port),
        transports: vec![transport],
        ..Default::default()
    };

    let source = PactSource::File(
        contract_path
            .to_str()
            .expect("contract path is valid UTF-8")
            .to_string(),
    );

    let options = VerificationOptions::<NullRequestFilterExecutor> {
        disable_ssl_verification: true,
        request_filter: None::<Arc<NullRequestFilterExecutor>>,
        ..Default::default()
    };

    let state_executor = Arc::new(StateExecutor {
        pool: app.pool.clone(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
    });

    let result = verify_provider_async(
        provider,
        vec![source],
        FilterInfo::None,
        vec![],
        &options,
        None,
        &state_executor,
        None,
    )
    .await
    .expect("pact verification execution failed");

    assert!(
        result.result,
        "Pact verification failed for {}. Errors: {:?}",
        contract_path.display(),
        result.errors,
    );
}

#[tokio::test]
#[ignore = "contract tests need provider state wiring — tracked separately"]
async fn verify_ios_contract() {
    let contract = contracts_dir().join("ios-backend.json");
    if !contract.exists() {
        panic!("iOS contract file not found at {}", contract.display());
    }

    let app = common::setup().await;
    verify_contract(&app, contract).await;
}

#[tokio::test]
#[ignore = "contract tests need provider state wiring — tracked separately"]
async fn verify_web_contract() {
    let contract = contracts_dir().join("web-backend.json");
    if !contract.exists() {
        panic!("Web contract file not found at {}", contract.display());
    }

    let app = common::setup().await;
    verify_contract(&app, contract).await;
}
