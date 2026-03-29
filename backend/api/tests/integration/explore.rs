// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

// ---------------------------------------------------------------------------
// GET /explore/metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_metrics_returns_static_sources() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/metrics",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let sources = json["sources"].as_array().unwrap();

    // Should have 5 source groups: health_records, checkins, labs, calendar, sleep
    assert_eq!(sources.len(), 5);
    assert_eq!(sources[0]["source"], "health_records");
    assert_eq!(sources[1]["source"], "checkins");
    assert_eq!(sources[2]["source"], "labs");
    assert_eq!(sources[3]["source"], "calendar");
    assert_eq!(sources[4]["source"], "sleep");

    // Health records should have 15 metrics
    let hr_metrics = sources[0]["metrics"].as_array().unwrap();
    assert_eq!(hr_metrics.len(), 15);

    // Checkins should have 5
    let ck_metrics = sources[1]["metrics"].as_array().unwrap();
    assert_eq!(ck_metrics.len(), 5);

    // Labs should be empty for a new user
    let lab_metrics = sources[2]["metrics"].as_array().unwrap();
    assert!(lab_metrics.is_empty());
}

#[tokio::test]
async fn test_explore_metrics_includes_lab_markers() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed a lab result
    let body = json!({
        "panel_date": "2026-03-15",
        "marker": "testosterone_total",
        "value": 650.0,
        "unit": "ng/dL",
        "source": "manual"
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/labs",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/metrics",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let lab_source = &json["sources"][2];
    let lab_metrics = lab_source["metrics"].as_array().unwrap();
    assert_eq!(lab_metrics.len(), 1);
    assert_eq!(lab_metrics[0]["field"], "testosterone_total");
}

#[tokio::test]
async fn test_explore_metrics_unauthenticated() {
    let app = common::setup().await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/metrics",
            "invalid-token",
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// GET /explore/series (single)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_series_checkin_daily() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed 3 days of checkins
    for day in 15..=17 {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "energy": day - 10, // 5, 6, 7
            "mood": 8
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/checkins",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=checkins&field=energy&start=2026-03-14T00:00:00Z&end=2026-03-18T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["source"], "checkins");
    assert_eq!(json["field"], "energy");
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 3);
}

#[tokio::test]
async fn test_explore_series_health_records() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed health records
    for i in 0..5 {
        let body = json!({
            "source": "manual",
            "record_type": "heart_rate",
            "value": 70.0 + (i as f64),
            "unit": "bpm",
            "start_time": format!("2026-03-{:02}T12:00:00Z", 15 + i)
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/health-records",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=health_records&field=heart_rate&start=2026-03-14T00:00:00Z&end=2026-03-21T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["source"], "health_records");
    assert_eq!(json["field"], "heart_rate");
    assert_eq!(json["unit"], "bpm");
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 5);
}

#[tokio::test]
async fn test_explore_series_weekly_aggregation() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed 14 days of checkins (should get 2-3 weekly buckets)
    for day in 1..=14 {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "energy": 7
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/checkins",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=checkins&field=energy&start=2026-02-28T00:00:00Z&end=2026-03-15T00:00:00Z&resolution=weekly",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    // 14 days should produce 2-3 weekly buckets (depending on week alignment)
    assert!(points.len() >= 2 && points.len() <= 3);
}

#[tokio::test]
async fn test_explore_series_invalid_source() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=invalid&field=energy&start=2026-03-01T00:00:00Z&end=2026-03-15T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_explore_series_invalid_field() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=checkins&field=nonexistent&start=2026-03-01T00:00:00Z&end=2026-03-15T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_explore_series_date_range_filtering() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed checkins on Mar 10 and Mar 20
    for day in [10, 20] {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "energy": 7
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/checkins",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    // Query only Mar 8-12 — should only return the Mar 10 data
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=checkins&field=energy&start=2026-03-08T00:00:00Z&end=2026-03-12T23:59:59Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
}

// ---------------------------------------------------------------------------
// POST /explore/series (batch)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_batch_series() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed data
    let checkin = json!({ "date": "2026-03-15", "energy": 7, "mood": 8 });
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&checkin),
        ))
        .await
        .unwrap();

    let body = json!({
        "metrics": [
            { "source": "checkins", "field": "energy" },
            { "source": "checkins", "field": "mood" }
        ],
        "start": "2026-03-14T00:00:00Z",
        "end": "2026-03-16T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/series",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let series = json["series"].as_array().unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0]["field"], "energy");
    assert_eq!(series[1]["field"], "mood");
}

#[tokio::test]
async fn test_explore_batch_series_too_many_metrics() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let metrics: Vec<serde_json::Value> = (0..9)
        .map(|_| json!({ "source": "checkins", "field": "energy" }))
        .collect();

    let body = json!({
        "metrics": metrics,
        "start": "2026-03-01T00:00:00Z",
        "end": "2026-03-15T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/series",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_explore_batch_series_invalid_metric() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metrics": [
            { "source": "checkins", "field": "energy" },
            { "source": "invalid_source", "field": "foo" }
        ],
        "start": "2026-03-01T00:00:00Z",
        "end": "2026-03-15T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/series",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Saved charts CRUD
// ---------------------------------------------------------------------------

fn valid_chart_body() -> serde_json::Value {
    json!({
        "name": "My Chart",
        "config": {
            "version": 1,
            "metrics": [
                { "source": "checkins", "field": "energy", "color": "#ff0000" }
            ],
            "range": { "preset": "30d" },
            "resolution": "daily"
        }
    })
}

#[tokio::test]
async fn test_create_chart() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let json = common::body_json(resp).await;
    assert_eq!(json["name"], "My Chart");
    assert!(json["id"].as_str().is_some());
}

#[tokio::test]
async fn test_list_charts() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a chart
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/charts",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let charts = json.as_array().unwrap();
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0]["name"], "My Chart");
}

#[tokio::test]
async fn test_get_chart_by_id() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();
    let created = common::body_json(create_resp).await;
    let chart_id = created["id"].as_str().unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["id"], chart_id);
    assert_eq!(json["name"], "My Chart");
}

#[tokio::test]
async fn test_update_chart() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();
    let created = common::body_json(create_resp).await;
    let chart_id = created["id"].as_str().unwrap();

    let update_body = json!({ "name": "Updated Chart" });
    let resp = app
        .app
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token,
            Some(&update_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["name"], "Updated Chart");
}

#[tokio::test]
async fn test_delete_chart() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();
    let created = common::body_json(create_resp).await;
    let chart_id = created["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify 404 on get
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_chart_idor_protection() {
    let app = common::setup().await;
    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    // User A creates a chart
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token_a,
            Some(&valid_chart_body()),
        ))
        .await
        .unwrap();
    let created = common::body_json(create_resp).await;
    let chart_id = created["id"].as_str().unwrap();

    // User B tries to get it — 404 (not 403, to avoid info leak)
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // User B tries to update it — 404
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token_b,
            Some(&json!({ "name": "hacked" })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // User B tries to delete it — 404
    let resp = app
        .app
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/explore/charts/{chart_id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_create_chart_invalid_config() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Invalid version
    let body = json!({
        "name": "Bad Chart",
        "config": {
            "version": 2,
            "metrics": [{ "source": "checkins", "field": "energy" }],
            "range": { "preset": "30d" },
            "resolution": "daily"
        }
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid metric source
    let body = json!({
        "name": "Bad Chart",
        "config": {
            "version": 1,
            "metrics": [{ "source": "invalid", "field": "foo" }],
            "range": { "preset": "30d" },
            "resolution": "daily"
        }
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/charts",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_chart_not_found() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let fake_id = uuid::Uuid::new_v4();
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/explore/charts/{fake_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Bug fix: checkins date filtering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_checkins_date_filtering() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create checkins on different dates
    for day in [10, 15, 20] {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "energy": 7
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/checkins",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    // Filter to Mar 12 - Mar 18 — should only return the Mar 15 checkin
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/checkins?start=2026-03-12&end=2026-03-18",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let items = json.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["date"], "2026-03-15");
}

// ---------------------------------------------------------------------------
// Bug fix: labs date filtering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_labs_date_filtering() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create labs on different dates
    for day in [10, 15, 20] {
        let body = json!({
            "panel_date": format!("2026-03-{day:02}"),
            "marker": "creatinine",
            "value": 1.0,
            "unit": "mg/dL",
            "source": "manual"
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/labs",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    // Filter to Mar 12 - Mar 18 — should only return the Mar 15 lab
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/labs?start=2026-03-12&end=2026-03-18",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let items = json.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["panel_date"], "2026-03-15");
}

// ---------------------------------------------------------------------------
// SSE events — basic auth validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_events_invalid_token() {
    let app = common::setup().await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/events?token=invalid-jwt",
            "dummy", // not used for SSE, token is in query param
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// Explore series — lab data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_series_lab_data() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed lab results
    for day in 10..=12 {
        let body = json!({
            "panel_date": format!("2026-03-{day:02}"),
            "marker": "creatinine",
            "value": 1.0 + (day as f64 - 10.0) * 0.1,
            "unit": "mg/dL",
            "source": "manual"
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/labs",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=labs&field=creatinine&start=2026-03-09T00:00:00Z&end=2026-03-13T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["source"], "labs");
    assert_eq!(json["field"], "creatinine");
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 3);
}

// ---------------------------------------------------------------------------
// Explore series — sleep data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_series_sleep_data() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed sleep records
    for day in 15..=17 {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "duration_minutes": 420 + (day - 15) * 10,
            "deep_minutes": 90,
            "rem_minutes": 100,
            "score": 80
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/sleep",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=sleep&field=duration_minutes&start=2026-03-14T00:00:00Z&end=2026-03-18T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["source"], "sleep");
    assert_eq!(json["field"], "duration_minutes");
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 3);
}

// ---------------------------------------------------------------------------
// GET /explore/interventions — intervention markers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_interventions_markers() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed interventions
    for day in 10..=12 {
        let body = json!({
            "substance": "BPC-157",
            "dose": 250.0,
            "unit": "mcg",
            "route": "subq",
            "administered_at": format!("2026-03-{day:02}T08:30:00Z")
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/interventions",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/interventions?start=2026-03-09T00:00:00Z&end=2026-03-13T00:00:00Z",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let markers = json.as_array().unwrap();
    assert_eq!(markers.len(), 3);
    assert_eq!(markers[0]["substance"], "BPC-157");
    assert_eq!(markers[0]["dose"], 250.0);
    assert_eq!(markers[0]["unit"], "mcg");
    assert_eq!(markers[0]["route"], "subq");
    // Verify no extra fields like id, user_id, notes, fasted
    assert!(markers[0].get("id").is_none());
    assert!(markers[0].get("user_id").is_none());
    assert!(markers[0].get("notes").is_none());
}

#[tokio::test]
async fn test_explore_interventions_date_filtering() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed interventions on different days
    for day in [5, 15, 25] {
        let body = json!({
            "substance": "TB-500",
            "dose": 2.5,
            "unit": "mg",
            "route": "subq",
            "administered_at": format!("2026-03-{day:02}T08:00:00Z")
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/interventions",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    // Query only Mar 10-20 — should return only the Mar 15 intervention
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/interventions?start=2026-03-10T00:00:00Z&end=2026-03-20T00:00:00Z",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let markers = json.as_array().unwrap();
    assert_eq!(markers.len(), 1);
}

#[tokio::test]
async fn test_explore_interventions_unauthenticated() {
    let app = common::setup().await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/interventions?start=2026-03-01T00:00:00Z&end=2026-03-31T00:00:00Z",
            "invalid-token",
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_explore_interventions_empty_result() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/interventions?start=2026-03-01T00:00:00Z&end=2026-03-31T00:00:00Z",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let markers = json.as_array().unwrap();
    assert!(markers.is_empty());
}

// ---------------------------------------------------------------------------
// GET /explore/metrics — observer_polls source
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_metrics_includes_observer_polls() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create an observer poll
    let body = json!({
        "name": "Daily Check",
        "dimensions": ["energy", "mood", "focus"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Fetch metrics — should now include observer_polls source
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/metrics",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let sources = json["sources"].as_array().unwrap();

    // Should have 6 source groups now (5 static + 1 observer_polls)
    assert_eq!(sources.len(), 6);
    let poll_source = &sources[5];
    assert_eq!(poll_source["source"], "observer_polls");
    assert_eq!(poll_source["label"], "Observer Polls");

    let poll_metrics = poll_source["metrics"].as_array().unwrap();
    assert_eq!(poll_metrics.len(), 3);

    // Verify metric fields have the right format
    let first_field = poll_metrics[0]["field"].as_str().unwrap();
    assert!(first_field.contains(':'));
    assert!(
        first_field.ends_with("energy")
            || first_field.ends_with("mood")
            || first_field.ends_with("focus")
    );
}

#[tokio::test]
async fn test_explore_metrics_no_observer_polls_when_user_has_none() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/metrics",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let sources = json["sources"].as_array().unwrap();

    // Should still have 5 sources (no observer_polls)
    assert_eq!(sources.len(), 5);
    assert!(sources.iter().all(|s| s["source"] != "observer_polls"));
}

// ---------------------------------------------------------------------------
// GET/POST /explore/series — observer_polls source
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_explore_series_observer_polls() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Create a poll
    let body = json!({
        "name": "Test Poll",
        "dimensions": ["energy", "mood"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let poll_json = common::body_json(resp).await;
    let poll_id = poll_json["id"].as_str().unwrap();

    // Create an invite and accept it with a second user
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let invite_json = common::body_json(resp).await;
    let invite_token = invite_json["invite_token"].as_str().unwrap();

    let (_observer_id, observer_token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &observer_token,
            Some(&json!({ "token": invite_token })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Submit responses for 3 days
    for day in 15..=17 {
        let body = json!({
            "date": format!("2026-03-{day:02}"),
            "scores": { "energy": day - 10, "mood": 8 }
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "PUT",
                &format!("/api/v1/observer-polls/{poll_id}/respond"),
                &observer_token,
                Some(&body),
            ))
            .await
            .unwrap();
        let status = resp.status().as_u16();
        assert!(
            status == 200 || status == 201,
            "failed to submit response for day {day}, got {status}"
        );
    }

    // Query the observer poll series as the poll owner
    let field = format!("{poll_id}:energy");
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=observer_polls&field={field}&start=2026-03-14T00:00:00Z&end=2026-03-18T00:00:00Z&resolution=daily"
            ),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["source"], "observer_polls");
    assert_eq!(json["unit"], "score");
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 3);
}

#[tokio::test]
async fn test_explore_series_observer_polls_not_owned() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    let (_other_id, other_token) = common::create_test_user(&app).await;

    // User A creates a poll
    let body = json!({
        "name": "Private Poll",
        "dimensions": ["energy"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let poll_json = common::body_json(resp).await;
    let poll_id = poll_json["id"].as_str().unwrap();

    // User B queries the poll series — should return empty (no data, not an error,
    // since the ownership check is in the SQL WHERE clause)
    let field = format!("{poll_id}:energy");
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=observer_polls&field={field}&start=2026-03-01T00:00:00Z&end=2026-03-31T00:00:00Z&resolution=daily"
            ),
            &other_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    assert!(points.is_empty());
}

#[tokio::test]
async fn test_explore_series_observer_polls_invalid_field_format() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // No colon separator
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=observer_polls&field=bad-field&start=2026-03-01T00:00:00Z&end=2026-03-31T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid UUID
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/explore/series?source=observer_polls&field=not-a-uuid:energy&start=2026-03-01T00:00:00Z&end=2026-03-31T00:00:00Z&resolution=daily",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_explore_batch_series_with_observer_polls() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a poll
    let body = json!({
        "name": "Batch Poll",
        "dimensions": ["energy", "mood"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let poll_json = common::body_json(resp).await;
    let poll_id = poll_json["id"].as_str().unwrap();

    // Batch request including observer_polls (no data, but should not error)
    let body = json!({
        "metrics": [
            { "source": "checkins", "field": "energy" },
            { "source": "observer_polls", "field": format!("{poll_id}:energy") }
        ],
        "start": "2026-03-01T00:00:00Z",
        "end": "2026-03-31T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/explore/series",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let series = json["series"].as_array().unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0]["source"], "checkins");
    assert_eq!(series[1]["source"], "observer_polls");
}
