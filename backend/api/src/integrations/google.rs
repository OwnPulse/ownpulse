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
    /// Access token lifetime in seconds. Absent from the login/signup flow's
    /// usage (short-lived id_token exchange consumed immediately) but
    /// required by callers that persist the access token for later use (the
    /// Google Calendar connect flow).
    #[serde(default)]
    pub expires_in: Option<i64>,
}

/// User profile information from Google's userinfo endpoint.
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
}

/// Exchange an authorization code for tokens via Google's OAuth2 token endpoint.
///
/// When `code_verifier` is `Some`, it is included in the request body for
/// PKCE flows (RFC 7636). Google verifies it against the `code_challenge`
/// that was sent in the original authorization request. For web flows where
/// CSRF is handled via the `oauth_state` cookie, pass `None`.
pub async fn exchange_code_for_tokens(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    token_url: &str,
    code_verifier: Option<&str>,
) -> Result<GoogleTokenResponse, String> {
    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("code", code),
    ];
    if let Some(verifier) = code_verifier {
        params.push(("code_verifier", verifier));
    }
    let response = client
        .post(token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token exchange request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unreadable body".into());
        return Err(format!("token exchange returned {status}: {body}"));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| format!("failed to parse token response: {e}"))
}

/// Exchange a refresh token for a new access token via Google's OAuth2 token
/// endpoint. Used by the Google Calendar sync job — the short-lived access
/// tokens issued for `calendar.readonly` expire in ~1 hour, so a background
/// job needs to refresh them itself rather than requiring the user to
/// re-authorize.
pub async fn refresh_access_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    token_url: &str,
) -> Result<GoogleTokenResponse, String> {
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
    ];

    let response = client
        .post(token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token refresh request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        // Never log/return the raw body here — Google's error responses can
        // echo back request parameters. Only the status is surfaced.
        let _ = response.text().await;
        return Err(format!("token refresh returned HTTP {status}"));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| format!("failed to parse token refresh response: {e}"))
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
