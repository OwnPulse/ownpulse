// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::Utc;
use regex::Regex;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::models::observer_poll::{
    AcceptInvite, CreatePoll, InviteResponse, ObserverPollView, PollMemberView, PollResponse,
    ResponsesQuery, SubmitResponse, UpdatePoll,
};

/// Reuse the email masking function from friends module.
fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() => {
            format!("{}***@{}", &local[..1], domain)
        }
        _ => "***".to_string(),
    }
}

/// Strip HTML tags from a string using a simple regex.
fn strip_html_tags(input: &str) -> String {
    static RE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"<[^>]*>").expect("invalid regex"));
    RE.replace_all(input, "").to_string()
}

/// Dimension name pattern: alphanumeric + underscore, 1-50 chars.
fn is_valid_dimension(dim: &str) -> bool {
    !dim.is_empty()
        && dim.len() <= 50
        && dim
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_dimensions(dimensions: &serde_json::Value) -> Result<Vec<String>, ApiError> {
    let arr = dimensions
        .as_array()
        .ok_or_else(|| ApiError::BadRequest("dimensions must be an array".to_string()))?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| ApiError::BadRequest("each dimension must be a string".to_string()))
        })
        .collect()
}

fn validate_create_poll(body: &CreatePoll) -> Result<(), ApiError> {
    if body.name.is_empty() || body.name.len() > 100 {
        return Err(ApiError::BadRequest(
            "name must be 1-100 characters".to_string(),
        ));
    }
    if let Some(ref prompt) = body.custom_prompt
        && prompt.len() > 500
    {
        return Err(ApiError::BadRequest(
            "custom_prompt must be at most 500 characters".to_string(),
        ));
    }
    if body.dimensions.is_empty() || body.dimensions.len() > 10 {
        return Err(ApiError::BadRequest(
            "dimensions must have 1-10 items".to_string(),
        ));
    }
    for dim in &body.dimensions {
        if !is_valid_dimension(dim) {
            return Err(ApiError::BadRequest(format!(
                "invalid dimension name: {dim}. Must be 1-50 alphanumeric/underscore characters"
            )));
        }
    }
    Ok(())
}

fn validate_scores(
    scores: &serde_json::Value,
    dimensions: &[String],
) -> Result<HashMap<String, i32>, ApiError> {
    let obj = scores
        .as_object()
        .ok_or_else(|| ApiError::BadRequest("scores must be a JSON object".to_string()))?;

    if obj.len() != dimensions.len() {
        return Err(ApiError::BadRequest(format!(
            "scores must contain exactly {} dimensions",
            dimensions.len()
        )));
    }

    let mut result = HashMap::new();

    for (key, value) in obj {
        if !dimensions.contains(key) {
            return Err(ApiError::BadRequest(format!(
                "unknown dimension in scores: {key}"
            )));
        }
        let v = value.as_i64().ok_or_else(|| {
            ApiError::BadRequest(format!("score for {key} must be an integer"))
        })?;
        if !(1..=10).contains(&v) {
            return Err(ApiError::BadRequest(format!(
                "score for {key} must be between 1 and 10"
            )));
        }
        result.insert(key.clone(), v as i32);
    }

    Ok(result)
}

/// POST /observer-polls
pub async fn create_poll(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(mut body): Json<CreatePoll>,
) -> Result<(StatusCode, Json<PollResponse>), ApiError> {
    // Strip HTML from custom_prompt
    if let Some(ref prompt) = body.custom_prompt {
        body.custom_prompt = Some(strip_html_tags(prompt));
    }

    validate_create_poll(&body)?;

    let dimensions_json = serde_json::to_value(&body.dimensions)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let poll = db::observer_polls::create_poll(
        &state.pool,
        user_id,
        &body.name,
        body.custom_prompt.as_deref(),
        &dimensions_json,
    )
    .await?;

    let response = PollResponse {
        id: poll.id,
        name: poll.name,
        custom_prompt: poll.custom_prompt,
        dimensions: body.dimensions,
        members: vec![],
        created_at: poll.created_at,
        deleted_at: poll.deleted_at,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /observer-polls
pub async fn list_polls(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<PollResponse>>, ApiError> {
    let polls = db::observer_polls::list_polls(&state.pool, user_id).await?;

    let mut responses = Vec::with_capacity(polls.len());
    for poll in polls {
        let dimensions = parse_dimensions(&poll.dimensions)?;
        responses.push(PollResponse {
            id: poll.id,
            name: poll.name,
            custom_prompt: poll.custom_prompt,
            dimensions,
            members: vec![],
            created_at: poll.created_at,
            deleted_at: poll.deleted_at,
        });
    }

    Ok(Json(responses))
}

/// GET /observer-polls/:id
pub async fn get_poll(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> Result<Json<PollResponse>, ApiError> {
    let poll = db::observer_polls::get_poll(&state.pool, poll_id, user_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let members = db::observer_polls::list_members(&state.pool, poll_id).await?;
    let dimensions = parse_dimensions(&poll.dimensions)?;

    let member_views: Vec<PollMemberView> = members
        .into_iter()
        .map(|m| PollMemberView {
            id: m.id,
            observer_email: m
                .observer_email
                .as_deref()
                .map(mask_email)
                .unwrap_or_else(|| "pending".to_string()),
            accepted_at: m.accepted_at,
            created_at: m.created_at,
        })
        .collect();

    Ok(Json(PollResponse {
        id: poll.id,
        name: poll.name,
        custom_prompt: poll.custom_prompt,
        dimensions,
        members: member_views,
        created_at: poll.created_at,
        deleted_at: poll.deleted_at,
    }))
}

/// PATCH /observer-polls/:id
pub async fn update_poll(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
    Json(mut body): Json<UpdatePoll>,
) -> Result<Json<PollResponse>, ApiError> {
    if let Some(ref name) = body.name
        && (name.is_empty() || name.len() > 100)
    {
        return Err(ApiError::BadRequest(
            "name must be 1-100 characters".to_string(),
        ));
    }
    if let Some(ref prompt) = body.custom_prompt {
        let stripped = strip_html_tags(prompt);
        if stripped.len() > 500 {
            return Err(ApiError::BadRequest(
                "custom_prompt must be at most 500 characters".to_string(),
            ));
        }
        body.custom_prompt = Some(stripped);
    }

    let poll = db::observer_polls::update_poll(
        &state.pool,
        poll_id,
        user_id,
        body.name.as_deref(),
        body.custom_prompt.as_deref(),
    )
    .await?
    .ok_or(ApiError::NotFound)?;

    let dimensions = parse_dimensions(&poll.dimensions)?;

    Ok(Json(PollResponse {
        id: poll.id,
        name: poll.name,
        custom_prompt: poll.custom_prompt,
        dimensions,
        members: vec![],
        created_at: poll.created_at,
        deleted_at: poll.deleted_at,
    }))
}

/// DELETE /observer-polls/:id
pub async fn delete_poll(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::observer_polls::soft_delete_poll(&state.pool, poll_id, user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /observer-polls/:id/invite
pub async fn create_invite(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> Result<(StatusCode, Json<InviteResponse>), ApiError> {
    // Verify poll ownership and not deleted
    if !db::observer_polls::poll_exists_for_user(&state.pool, poll_id, user_id).await? {
        return Err(ApiError::NotFound);
    }

    let (invite_token, invite_expires_at) =
        db::observer_polls::create_invite(&state.pool, poll_id).await?;

    let invite_url = format!(
        "{}/observer-polls/accept?token={}",
        state.config.web_origin, invite_token
    );

    Ok((
        StatusCode::CREATED,
        Json(InviteResponse {
            invite_token,
            invite_expires_at,
            invite_url,
        }),
    ))
}

/// GET /observer-polls/:id/responses
pub async fn list_responses(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
    Query(query): Query<ResponsesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut responses = db::observer_polls::list_responses_for_owner(
        &state.pool,
        poll_id,
        user_id,
        query.start,
        query.end,
    )
    .await?;

    // If no responses returned, check if the poll exists and is owned by user
    if responses.is_empty()
        && !db::observer_polls::poll_exists_for_user(&state.pool, poll_id, user_id).await?
    {
        return Err(ApiError::NotFound);
    }

    // Mask emails in responses
    for resp in &mut responses {
        resp.observer_email = resp
            .observer_email
            .as_deref()
            .map(mask_email);
    }

    Ok(Json(serde_json::json!({ "responses": responses })))
}

/// POST /observer-polls/accept
pub async fn accept_invite(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<AcceptInvite>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = db::observer_polls::accept_invite(&state.pool, body.token, user_id).await?;

    match result {
        Some(_poll) => Ok(Json(json!({"status": "accepted"}))),
        None => Ok(Json(json!({"status": "acknowledged"}))),
    }
}

/// GET /observer-polls/my-polls
pub async fn my_polls(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<ObserverPollView>>, ApiError> {
    let polls = db::observer_polls::list_observer_polls(&state.pool, user_id).await?;

    let views: Vec<ObserverPollView> = polls
        .into_iter()
        .map(|p| {
            let dimensions = p
                .dimensions
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            ObserverPollView {
                id: p.id,
                owner_display: p
                    .owner_email
                    .as_deref()
                    .map(mask_email)
                    .unwrap_or_else(|| "***".to_string()),
                name: p.name,
                custom_prompt: p.custom_prompt,
                dimensions,
            }
        })
        .collect();

    Ok(Json(views))
}

/// PUT /observer-polls/:id/respond
pub async fn submit_response(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
    Json(body): Json<SubmitResponse>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    // Verify caller is accepted member
    let member_id = db::observer_polls::get_accepted_member_id(&state.pool, poll_id, user_id)
        .await?
        .ok_or(ApiError::Forbidden)?;

    // Get poll dimensions
    let dimensions_json = db::observer_polls::get_poll_dimensions(&state.pool, poll_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let dimensions = parse_dimensions(&dimensions_json)?;

    // Validate scores against dimensions
    validate_scores(&body.scores, &dimensions)?;

    // Validate date is not in the future
    let today = Utc::now().date_naive();
    if body.date > today {
        return Err(ApiError::BadRequest(
            "date cannot be in the future".to_string(),
        ));
    }

    let (response, is_new) =
        db::observer_polls::upsert_response(&state.pool, poll_id, member_id, body.date, &body.scores)
            .await?;

    let status = if is_new {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((
        status,
        Json(serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))?),
    ))
}

/// GET /observer-polls/:id/my-responses
pub async fn my_responses(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(poll_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let member_id = db::observer_polls::get_accepted_member_id(&state.pool, poll_id, user_id)
        .await?
        .ok_or(ApiError::Forbidden)?;

    let responses = db::observer_polls::list_my_responses(&state.pool, member_id).await?;

    Ok(Json(serde_json::json!({ "responses": responses })))
}

/// DELETE /observer-polls/responses/:response_id
pub async fn delete_response(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(response_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::observer_polls::delete_response(&state.pool, response_id, user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /observer-polls/export
pub async fn export_responses(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let responses = db::observer_polls::export_observer_responses(&state.pool, user_id).await?;

    Ok(Json(serde_json::json!({ "responses": responses })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(
            strip_html_tags("<script>alert(1)</script>"),
            "alert(1)"
        );
        assert_eq!(strip_html_tags("no tags"), "no tags");
        assert_eq!(
            strip_html_tags("<b>bold</b> and <i>italic</i>"),
            "bold and italic"
        );
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_is_valid_dimension() {
        assert!(is_valid_dimension("energy"));
        assert!(is_valid_dimension("mood_score"));
        assert!(is_valid_dimension("A1"));
        assert!(!is_valid_dimension(""));
        assert!(!is_valid_dimension("has space"));
        assert!(!is_valid_dimension("has-dash"));
        assert!(!is_valid_dimension(&"a".repeat(51)));
    }

    #[test]
    fn test_mask_email() {
        assert_eq!(mask_email("tony@example.com"), "t***@example.com");
        assert_eq!(mask_email("a@example.com"), "a***@example.com");
        assert_eq!(mask_email("@broken.com"), "***");
        assert_eq!(mask_email("noatsign"), "***");
    }

    #[test]
    fn test_validate_create_poll_valid() {
        let body = CreatePoll {
            name: "My poll".to_string(),
            custom_prompt: None,
            dimensions: vec!["energy".to_string(), "mood".to_string()],
        };
        assert!(validate_create_poll(&body).is_ok());
    }

    #[test]
    fn test_validate_create_poll_empty_name() {
        let body = CreatePoll {
            name: "".to_string(),
            custom_prompt: None,
            dimensions: vec!["energy".to_string()],
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_create_poll_name_too_long() {
        let body = CreatePoll {
            name: "a".repeat(101),
            custom_prompt: None,
            dimensions: vec!["energy".to_string()],
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_create_poll_empty_dimensions() {
        let body = CreatePoll {
            name: "poll".to_string(),
            custom_prompt: None,
            dimensions: vec![],
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_create_poll_too_many_dimensions() {
        let body = CreatePoll {
            name: "poll".to_string(),
            custom_prompt: None,
            dimensions: (0..11).map(|i| format!("dim_{i}")).collect(),
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_create_poll_invalid_dimension_chars() {
        let body = CreatePoll {
            name: "poll".to_string(),
            custom_prompt: None,
            dimensions: vec!["has-dash".to_string()],
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_create_poll_prompt_too_long() {
        let body = CreatePoll {
            name: "poll".to_string(),
            custom_prompt: Some("a".repeat(501)),
            dimensions: vec!["energy".to_string()],
        };
        assert!(validate_create_poll(&body).is_err());
    }

    #[test]
    fn test_validate_scores_valid() {
        let scores = json!({"energy": 5, "mood": 8});
        let dims = vec!["energy".to_string(), "mood".to_string()];
        assert!(validate_scores(&scores, &dims).is_ok());
    }

    #[test]
    fn test_validate_scores_unknown_key() {
        let scores = json!({"energy": 5, "unknown": 3});
        let dims = vec!["energy".to_string(), "mood".to_string()];
        assert!(validate_scores(&scores, &dims).is_err());
    }

    #[test]
    fn test_validate_scores_value_zero() {
        let scores = json!({"energy": 0});
        let dims = vec!["energy".to_string()];
        assert!(validate_scores(&scores, &dims).is_err());
    }

    #[test]
    fn test_validate_scores_value_eleven() {
        let scores = json!({"energy": 11});
        let dims = vec!["energy".to_string()];
        assert!(validate_scores(&scores, &dims).is_err());
    }

    #[test]
    fn test_validate_scores_not_object() {
        let scores = json!([1, 2, 3]);
        let dims = vec!["energy".to_string()];
        assert!(validate_scores(&scores, &dims).is_err());
    }

    #[test]
    fn test_validate_scores_non_integer() {
        let scores = json!({"energy": "high"});
        let dims = vec!["energy".to_string()];
        assert!(validate_scores(&scores, &dims).is_err());
    }
}
