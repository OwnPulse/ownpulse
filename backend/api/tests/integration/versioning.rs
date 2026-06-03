// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the API versioning scaffold.
//!
//! Verifies that the `/api/v2` namespace is mounted but currently empty: any
//! request under it falls through to a clean 404 (not a 500 or a panic), while
//! the existing `/api/v1` surface continues to respond.

use axum::body::Body;
use http::Request;
use tower::ServiceExt;

use crate::common;

/// The v2 namespace is mounted but has no routes yet, so any path under it
/// returns 404.
#[tokio::test]
async fn test_v2_root_returns_404() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v2/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

/// An arbitrary nested path under v2 also returns a clean 404.
#[tokio::test]
async fn test_v2_arbitrary_path_returns_404() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v2/anything/here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

/// Mounting v2 does not disturb the existing v1 surface — the v1 health check
/// still responds 200.
#[tokio::test]
async fn test_v1_health_still_works() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
