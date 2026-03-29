// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use tower::ServiceExt;

use crate::common;

/// Build a multipart request with a file field.
fn multipart_upload_request(
    uri: &str,
    token: &str,
    filename: &str,
    content: &[u8],
) -> Request<Body> {
    let boundary = "----TestBoundary123456";
    let mut body_bytes = Vec::new();

    body_bytes.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body_bytes.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body_bytes.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body_bytes.extend_from_slice(content);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body_bytes))
        .unwrap()
}

/// Helper to seed snp_annotations for tests that need interpretations.
async fn seed_annotations(app: &common::TestApp) {
    api::db::snp_seed::seed_annotations(&app.pool)
        .await
        .expect("failed to seed annotations");
}

fn test_23andme_fixture() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/genetics/test_23andme.txt");
    std::fs::read(path).expect("failed to read 23andMe fixture")
}

fn test_ancestry_fixture() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/genetics/test_ancestry.txt");
    std::fs::read(path).expect("failed to read AncestryDNA fixture")
}

// ==========================================
// Upload tests
// ==========================================

#[tokio::test]
async fn test_upload_23andme() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    seed_annotations(&app).await;

    let content = test_23andme_fixture();
    let req = multipart_upload_request(
        "/api/v1/genetics/upload",
        &token,
        "test_23andme.txt",
        &content,
    );

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201, "upload should return 201 Created");

    let body = common::body_json(resp).await;
    assert!(body["total_variants"].as_i64().unwrap() > 50);
    assert_eq!(body["format"], "23andme");
    assert_eq!(body["source"], "23andMe");
    // The fixture has one duplicate rsid (rs80338939 appears twice), so at most 1 dup
    assert!(body["duplicates_skipped"].as_i64().unwrap() <= 1);
}

#[tokio::test]
async fn test_upload_ancestrydna() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let content = test_ancestry_fixture();
    let req = multipart_upload_request(
        "/api/v1/genetics/upload",
        &token,
        "test_ancestry.txt",
        &content,
    );

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201);

    let body = common::body_json(resp).await;
    assert!(body["total_variants"].as_i64().unwrap() > 50);
    assert_eq!(body["format"], "ancestrydna");
    assert_eq!(body["source"], "AncestryDNA");
}

#[tokio::test]
async fn test_upload_duplicate_skips() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let content = test_23andme_fixture();

    // First upload
    let req = multipart_upload_request(
        "/api/v1/genetics/upload",
        &token,
        "test_23andme.txt",
        &content,
    );
    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201);
    let _body1 = common::body_json(resp).await;

    // Second upload — all should be duplicates
    let req = multipart_upload_request(
        "/api/v1/genetics/upload",
        &token,
        "test_23andme.txt",
        &content,
    );
    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201);

    let body2 = common::body_json(resp).await;
    assert_eq!(body2["new_variants"], 0);
    // All parsed variants should be skipped since they already exist
    assert!(body2["duplicates_skipped"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_upload_empty_file_returns_400() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "empty.txt", b"");

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_invalid_file_returns_400() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let req = multipart_upload_request(
        "/api/v1/genetics/upload",
        &token,
        "garbage.txt",
        b"this is not a genetic data file at all\nno structure here\n",
    );

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_upload_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/genetics/upload")
        .header("content-type", "multipart/form-data; boundary=test")
        .body(Body::from("--test--\r\n"))
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ==========================================
// List tests
// ==========================================

#[tokio::test]
async fn test_list_with_pagination() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Upload data
    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    // List page 1 with per_page=10
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics?page=1&per_page=10",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert_eq!(body["records"].as_array().unwrap().len(), 10);
    assert!(body["total"].as_i64().unwrap() > 10);
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 10);
}

#[tokio::test]
async fn test_list_filter_by_chromosome() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics?chromosome=1",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let records = body["records"].as_array().unwrap();
    assert!(!records.is_empty());
    for record in records {
        assert_eq!(record["chromosome"], "1");
    }
}

#[tokio::test]
async fn test_list_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/genetics")
        .body(Body::empty())
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ==========================================
// Summary tests
// ==========================================

#[tokio::test]
async fn test_summary_with_data() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    seed_annotations(&app).await;

    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/summary",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert!(body["total_variants"].as_i64().unwrap() > 0);
    assert_eq!(body["source"], "23andMe");
    assert!(body["uploaded_at"].as_str().is_some());
    assert!(body["chromosomes"].as_object().is_some());
    assert!(body["annotated_count"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_summary_empty() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/summary",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert_eq!(body["total_variants"], 0);
    assert!(body["source"].is_null());
}

#[tokio::test]
async fn test_summary_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/genetics/summary")
        .body(Body::empty())
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ==========================================
// Interpretations tests
// ==========================================

#[tokio::test]
async fn test_interpretations_returns_annotated_variants() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    seed_annotations(&app).await;

    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/interpretations",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;

    let interpretations = body["interpretations"].as_array().unwrap();
    assert!(!interpretations.is_empty());

    // Check structure of first interpretation
    let first = &interpretations[0];
    assert!(first["rsid"].as_str().is_some());
    assert!(first["category"].as_str().is_some());
    assert!(first["title"].as_str().is_some());
    assert!(first["summary"].as_str().is_some());
    assert!(first["risk_level"].as_str().is_some());
    assert!(first["significance"].as_str().is_some());
    assert!(first["evidence_level"].as_str().is_some());
    assert!(first["source"].as_str().is_some());

    // Check disclaimer is present
    assert!(
        body["disclaimer"]
            .as_str()
            .unwrap()
            .contains("educational purposes")
    );

    // Verify MTHFR C677T interpretation is present (CT genotype in fixture)
    let mthfr = interpretations.iter().find(|i| i["rsid"] == "rs1801133");
    assert!(mthfr.is_some(), "MTHFR interpretation should be present");
    let mthfr = mthfr.unwrap();
    assert_eq!(mthfr["user_genotype"], "CT");
    assert_eq!(mthfr["risk_level"], "moderate");
}

#[tokio::test]
async fn test_interpretations_filter_by_category() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    seed_annotations(&app).await;

    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/interpretations?category=pharmacogenomics",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let interpretations = body["interpretations"].as_array().unwrap();
    for interp in interpretations {
        assert_eq!(interp["category"], "pharmacogenomics");
    }
}

#[tokio::test]
async fn test_interpretations_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/genetics/interpretations")
        .body(Body::empty())
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ==========================================
// Delete tests
// ==========================================

#[tokio::test]
async fn test_delete_all_with_confirmation() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Upload
    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    // Verify data exists
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/summary",
            &token,
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert!(body["total_variants"].as_i64().unwrap() > 0);

    // Delete
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/genetics",
            &token,
            Some(&serde_json::json!({"confirm": true})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify data is gone
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/summary",
            &token,
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total_variants"], 0);
}

#[tokio::test]
async fn test_delete_without_confirmation_returns_400() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/genetics",
            &token,
            Some(&serde_json::json!({"confirm": false})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_delete_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/genetics")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"confirm":true}"#))
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ==========================================
// Cross-user isolation test
// ==========================================

#[tokio::test]
async fn test_user_isolation() {
    let app = common::setup().await;
    let (_user1_id, token1) = common::create_test_user(&app).await;
    let (_user2_id, token2) = common::create_test_user(&app).await;

    // User 1 uploads
    let content = test_23andme_fixture();
    let req = multipart_upload_request("/api/v1/genetics/upload", &token1, "test.txt", &content);
    app.app.clone().oneshot(req).await.unwrap();

    // User 2 should see no data
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics/summary",
            &token2,
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total_variants"], 0);

    // User 2 list should be empty
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/genetics",
            &token2,
            None,
        ))
        .await
        .unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body["total"], 0);
}
