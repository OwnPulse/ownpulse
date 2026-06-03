// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use tower::ServiceExt;

use crate::common;

/// The CORS preflight (OPTIONS) for a request carrying `X-App-Version` must be
/// allowed, otherwise browsers block every web-client API call. This pins the
/// production `cors_layer` allowlist against the header sent by the web client.
#[tokio::test]
async fn preflight_allows_x_app_version_header() {
    let test_app = common::setup().await;

    let request = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/health")
        .header("origin", "http://localhost:5173")
        .header("access-control-request-method", "GET")
        .header("access-control-request-headers", "x-app-version")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.oneshot(request).await.unwrap();

    let allow_headers = response
        .headers()
        .get("access-control-allow-headers")
        .expect("preflight response missing Access-Control-Allow-Headers")
        .to_str()
        .unwrap()
        .to_ascii_lowercase();

    assert!(
        allow_headers.contains("x-app-version"),
        "Access-Control-Allow-Headers must include x-app-version, got: {allow_headers}"
    );
}
