// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_saved_medicine() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "caffeine",
        "dose": 200.0,
        "unit": "mg",
        "route": "oral"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["substance"], "caffeine");
    assert_eq!(json["dose"], 200.0);
    assert_eq!(json["unit"], "mg");
    assert_eq!(json["route"], "oral");
    assert_eq!(json["sort_order"], 0);
    assert!(json["id"].is_string());
    assert!(json["created_at"].is_string());
}

#[tokio::test]
async fn test_list_saved_medicines() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create two medicines with different sort_order
    let body1 = json!({
        "substance": "magnesium",
        "dose": 400.0,
        "unit": "mg"
    });
    let body2 = json!({
        "substance": "zinc",
        "dose": 15.0,
        "unit": "mg"
    });

    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token,
            Some(&body1),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token,
            Some(&body2),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);

    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/saved-medicines",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert_eq!(items.len(), 2);
    // Both have sort_order 0, so ordered by created_at — magnesium first
    assert_eq!(items[0]["substance"], "magnesium");
    assert_eq!(items[1]["substance"], "zinc");
}

#[tokio::test]
async fn test_update_saved_medicine() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "vitamin_d",
        "dose": 2000.0,
        "unit": "IU"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // Update dose
    let update_body = json!({"dose": 5000.0});
    let update_resp = app
        .app
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/saved-medicines/{id}"),
            &token,
            Some(&update_body),
        ))
        .await
        .unwrap();

    assert_eq!(update_resp.status(), 200);
    let updated = common::body_json(update_resp).await;
    assert_eq!(updated["dose"], 5000.0);
    assert_eq!(updated["substance"], "vitamin_d");
    assert_eq!(updated["unit"], "IU");
}

#[tokio::test]
async fn test_delete_saved_medicine() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "melatonin",
        "dose": 3.0,
        "unit": "mg",
        "route": "oral"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // Delete
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/saved-medicines/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // Verify list is empty
    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/saved-medicines",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_cannot_access_other_users_medicines() {
    let app = common::setup().await;
    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    // User A creates a saved medicine
    let body = json!({
        "substance": "secret_supplement",
        "dose": 100.0,
        "unit": "mg"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/saved-medicines",
            &token_a,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // User B cannot see it in list
    let list_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/saved-medicines",
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(items.is_empty());

    // User B cannot update it
    let update_body = json!({"substance": "hacked"});
    let update_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/saved-medicines/{id}"),
            &token_b,
            Some(&update_body),
        ))
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 404);

    // User B cannot delete it
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/saved-medicines/{id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 404);
}
