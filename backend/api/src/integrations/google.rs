// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Google OAuth2 HTTP client for token exchange and user info retrieval.

use serde::Deserialize;

/// Response from Google's OAuth2 token endpoint.
#[derive(Debug, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
}

/// User profile information from Google's userinfo endpoint.
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
}

/// Exchange an authorization code for tokens via Google's OAuth2 token endpoint.
pub async fn exchange_code_for_tokens(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    token_url: &str,
) -> Result<GoogleTokenResponse, String> {
    let response = client
        .post(token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("code", code),
        ])
        .send()
        .await
        .map_err(|e| format!("token exchange request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unreadable body".into());
        return Err(format!(
            "token exchange returned {status}: {body}"
        ));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| format!("failed to parse token response: {e}"))
}

/// Fetch the authenticated user's profile from Google's userinfo endpoint.
pub async fn fetch_user_info(
    client: &reqwest::Client,
    access_token: &str,
    userinfo_url: &str,
) -> Result<GoogleUserInfo, String> {
    let response = client
        .get(userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("userinfo request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unreadable body".into());
        return Err(format!("userinfo returned {status}: {body}"));
    }

    response
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| format!("failed to parse userinfo response: {e}"))
}
