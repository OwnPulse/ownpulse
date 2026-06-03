// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration test for the `http_request_duration_seconds` histogram.
//!
//! Builds a router with the same observability middleware and Prometheus
//! exporter wiring used by `build_app`, drives a request through it, and
//! asserts the rendered `/metrics` output contains the request-duration
//! histogram with the expected labels.
//!
//! This is the only test in the integration binary that installs the global
//! `metrics` recorder (via `observability::build_metrics`). All other tests
//! use `build_app_without_metrics`, so there is no double-install conflict.

use axum::Router;
use axum::routing::get;
use tower::ServiceExt;

use crate::common;

/// A handler with a route parameter, used to confirm the `route` label uses the
/// matched route pattern (`/echo/:id`) rather than the raw path with the id.
async fn echo() -> &'static str {
    "ok"
}

#[tokio::test]
async fn metrics_endpoint_exposes_request_duration_histogram() {
    let handle = api::observability::build_metrics();

    let metrics_handle = handle.clone();
    let app: Router = Router::new()
        .route("/echo/:id", get(echo))
        .route(
            "/metrics",
            get(move || {
                let h = metrics_handle.clone();
                async move { h.render() }
            }),
        )
        .layer(axum::middleware::from_fn(
            api::observability::record_request_metrics,
        ));

    // Drive a request through the instrumented route. The id segment varies but
    // must not appear in the `route` label.
    let response = app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/echo/abc-123")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Scrape the metrics endpoint.
    let metrics_response = app
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics_response.status(), 200);

    let body = common::body_string(metrics_response).await;

    // The histogram metric is present.
    assert!(
        body.contains("http_request_duration_seconds"),
        "metrics output is missing http_request_duration_seconds:\n{body}"
    );
    // It is exported as a histogram (buckets), not a summary.
    assert!(
        body.contains("http_request_duration_seconds_bucket"),
        "http_request_duration_seconds is not a histogram (no _bucket lines):\n{body}"
    );
    // The route label uses the matched pattern, not the concrete id.
    assert!(
        body.contains("route=\"/echo/:id\""),
        "expected matched-route label route=\"/echo/:id\":\n{body}"
    );
    assert!(
        !body.contains("abc-123"),
        "raw path id leaked into a metric label (cardinality risk):\n{body}"
    );
    // The method and status_class labels are recorded.
    assert!(
        body.contains("method=\"GET\""),
        "missing method label:\n{body}"
    );
    assert!(
        body.contains("status_class=\"2xx\""),
        "missing status_class=\"2xx\" label:\n{body}"
    );
}
