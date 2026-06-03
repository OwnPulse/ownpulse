// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! HTTP request observability.
//!
//! Installs the process-wide Prometheus recorder used by the service and
//! provides a middleware that records a per-request latency histogram named
//! `http_request_duration_seconds` with the labels `route`, `method`, and
//! `status_class` (`2xx`/`4xx`/`5xx`).
//!
//! All of the service's metrics — the existing `ownpulse_app_*` /
//! `healthkit_*` counters emitted through the `metrics` facade and the
//! request-duration histogram defined here — are recorded into this single
//! recorder and exposed by one `/metrics` endpoint. Using the
//! [`metrics`]-facade recorder directly (rather than a second exporter) keeps
//! the existing application counters and the HTTP metrics in the same
//! exposition.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

/// Metric name for the per-request latency histogram. Matches the convention
/// expected by the dashboards: `http_request_duration_seconds{route, method,
/// status_class}`.
pub const HTTP_REQUEST_DURATION_SECONDS: &str = "http_request_duration_seconds";

/// Standard Prometheus latency buckets, in seconds. Without explicit buckets a
/// histogram is exported as a summary; configuring them makes the metric a true
/// histogram with `_bucket` series.
const SECONDS_DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Install the process-wide Prometheus recorder and return a handle that
/// renders the exposition format for the `/metrics` endpoint.
///
/// Histogram buckets are configured for [`HTTP_REQUEST_DURATION_SECONDS`] so it
/// is exported as a histogram rather than a summary. This installs the global
/// [`metrics`] recorder, so it must be called exactly once per process.
pub fn build_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(HTTP_REQUEST_DURATION_SECONDS.to_string()),
            SECONDS_DURATION_BUCKETS,
        )
        .expect("valid bucket configuration for request duration metric")
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// Map an HTTP status code to a low-cardinality `status_class` label.
fn status_class(status: u16) -> &'static str {
    match status {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        _ => "5xx",
    }
}

/// Middleware that records the `http_request_duration_seconds` histogram for
/// every request.
///
/// The `route` label uses the matched route pattern (e.g. `/api/v1/health`)
/// rather than the raw path, so path parameters such as record IDs do not
/// create unbounded label cardinality. Requests that do not match any route
/// (404s) are recorded under the `unmatched` route label.
pub async fn record_request_metrics(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();

    metrics::histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        "route" => route,
        "method" => method.as_str().to_owned(),
        "status_class" => status_class(response.status().as_u16()),
    )
    .record(elapsed);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_class_buckets_by_range() {
        assert_eq!(status_class(100), "1xx");
        assert_eq!(status_class(200), "2xx");
        assert_eq!(status_class(204), "2xx");
        assert_eq!(status_class(301), "3xx");
        assert_eq!(status_class(400), "4xx");
        assert_eq!(status_class(404), "4xx");
        assert_eq!(status_class(500), "5xx");
        assert_eq!(status_class(503), "5xx");
    }
}
