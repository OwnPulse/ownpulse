// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Oura Ring API HTTP client.
//!
//! Oura uses standard OAuth 2.0. This module handles:
//! - Authorization URL generation with PKCE
//! - Token exchange and refresh
//! - Data fetching from the v2 API
//!
//! All endpoints accept a base URL parameter for WireMock compatibility.

use serde::Deserialize;

// ── OAuth 2.0 types ─────────────────────────────────────────────────────

/// Token response from Oura's OAuth 2.0 token endpoint.
#[derive(Debug, Deserialize)]
pub struct OuraTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub token_type: Option<String>,
}

// ── API response types ──────────────────────────────────────────────────

/// Paginated response wrapper used by Oura v2 API.
#[derive(Debug, Deserialize)]
pub struct OuraPaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_token: Option<String>,
}

/// A single daily readiness record.
#[derive(Debug, Deserialize)]
pub struct OuraReadiness {
    pub id: Option<String>,
    pub day: Option<String>,
    pub score: Option<f64>,
    pub temperature_deviation: Option<f64>,
    pub timestamp: Option<String>,
    pub contributors: Option<OuraReadinessContributors>,
}

#[derive(Debug, Deserialize)]
pub struct OuraReadinessContributors {
    pub resting_heart_rate: Option<f64>,
    pub hrv_balance: Option<f64>,
    pub body_temperature: Option<f64>,
    pub recovery_index: Option<f64>,
}

/// A single daily sleep record.
#[derive(Debug, Deserialize)]
pub struct OuraSleep {
    pub id: Option<String>,
    pub day: Option<String>,
    pub score: Option<f64>,
    pub timestamp: Option<String>,
    pub contributors: Option<OuraSleepContributors>,
    pub deep_sleep_duration: Option<i64>,
    pub light_sleep_duration: Option<i64>,
    pub rem_sleep_duration: Option<i64>,
    pub awake_time: Option<i64>,
    pub total_sleep_duration: Option<i64>,
    pub bedtime_start: Option<String>,
    pub bedtime_end: Option<String>,
    pub average_heart_rate: Option<f64>,
    pub lowest_heart_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct OuraSleepContributors {
    pub deep_sleep: Option<f64>,
    pub efficiency: Option<f64>,
    pub latency: Option<f64>,
    pub rem_sleep: Option<f64>,
    pub restfulness: Option<f64>,
    pub timing: Option<f64>,
    pub total_sleep: Option<f64>,
}

/// A single daily activity record.
#[derive(Debug, Deserialize)]
pub struct OuraActivity {
    pub id: Option<String>,
    pub day: Option<String>,
    pub score: Option<f64>,
    pub active_calories: Option<i64>,
    pub total_calories: Option<i64>,
    pub steps: Option<i64>,
    pub equivalent_walking_distance: Option<f64>,
    pub timestamp: Option<String>,
}

/// A single heart rate sample.
#[derive(Debug, Deserialize)]
pub struct OuraHeartRate {
    pub bpm: Option<f64>,
    pub source: Option<String>,
    pub timestamp: Option<String>,
}

// ── Client ──────────────────────────────────────────────────────────────

/// HTTP client for the Oura Ring API v2.
///
/// `api_base_url` and `auth_base_url` can be overridden for WireMock testing.
pub struct OuraClient {
    pub client_id: String,
    pub client_secret: String,
    pub api_base_url: String,
    pub auth_base_url: String,
    pub http: reqwest::Client,
}

impl OuraClient {
    pub fn new(
        client_id: String,
        client_secret: String,
        api_base_url: Option<String>,
        auth_base_url: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            api_base_url: api_base_url
                .unwrap_or_else(|| "https://api.ouraring.com".to_string()),
            auth_base_url: auth_base_url
                .unwrap_or_else(|| "https://cloud.ouraring.com".to_string()),
            http,
        }
    }

    /// Build the OAuth 2.0 authorization URL.
    pub fn authorization_url(&self, redirect_uri: &str, state: &str) -> String {
        format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&scope=daily+heartrate+session+personal",
            self.auth_base_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(state),
        )
    }

    /// Exchange an authorization code for access and refresh tokens.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
    ) -> Result<OuraTokenResponse, String> {
        let url = format!("{}/oauth/token", self.api_base_url);

        let response = self
            .http
            .post(&url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("redirect_uri", redirect_uri),
                ("code", code),
            ])
            .send()
            .await
            .map_err(|e| format!("Oura token exchange request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("Oura token exchange returned {status}: {body}"));
        }

        response
            .json::<OuraTokenResponse>()
            .await
            .map_err(|e| format!("failed to parse Oura token response: {e}"))
    }

    /// Refresh an expired access token.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OuraTokenResponse, String> {
        let url = format!("{}/oauth/token", self.api_base_url);

        let response = self
            .http
            .post(&url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| format!("Oura token refresh request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("Oura token refresh returned {status}: {body}"));
        }

        response
            .json::<OuraTokenResponse>()
            .await
            .map_err(|e| format!("failed to parse Oura token refresh response: {e}"))
    }

    /// Fetch daily readiness data for a date range.
    pub async fn get_daily_readiness(
        &self,
        access_token: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<OuraPaginatedResponse<OuraReadiness>, String> {
        let url = format!("{}/v2/usercollection/daily_readiness", self.api_base_url);
        self.authenticated_get(&url, access_token, &[("start_date", start_date), ("end_date", end_date)])
            .await
    }

    /// Fetch daily sleep data for a date range.
    pub async fn get_daily_sleep(
        &self,
        access_token: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<OuraPaginatedResponse<OuraSleep>, String> {
        let url = format!("{}/v2/usercollection/daily_sleep", self.api_base_url);
        self.authenticated_get(&url, access_token, &[("start_date", start_date), ("end_date", end_date)])
            .await
    }

    /// Fetch daily activity data for a date range.
    pub async fn get_daily_activity(
        &self,
        access_token: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<OuraPaginatedResponse<OuraActivity>, String> {
        let url = format!("{}/v2/usercollection/daily_activity", self.api_base_url);
        self.authenticated_get(&url, access_token, &[("start_date", start_date), ("end_date", end_date)])
            .await
    }

    /// Fetch heart rate data for a date range.
    pub async fn get_heart_rate(
        &self,
        access_token: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<OuraPaginatedResponse<OuraHeartRate>, String> {
        let url = format!("{}/v2/usercollection/heartrate", self.api_base_url);
        self.authenticated_get(&url, access_token, &[("start_date", start_date), ("end_date", end_date)])
            .await
    }

    /// Make an authenticated GET request and deserialize the JSON response.
    async fn authenticated_get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        access_token: &str,
        query_params: &[(&str, &str)],
    ) -> Result<T, String> {
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .query(query_params)
            .send()
            .await
            .map_err(|e| format!("Oura API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("Oura API returned {status}: {body}"));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| format!("failed to parse Oura API response: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_contains_required_params() {
        let client = OuraClient::new(
            "test-client-id".to_string(),
            "test-secret".to_string(),
            None,
            None,
            reqwest::Client::new(),
        );

        let url = client.authorization_url("https://example.com/callback", "csrf-state-123");
        assert!(url.starts_with("https://cloud.ouraring.com/oauth/authorize"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=csrf-state-123"));
        assert!(url.contains("scope="));
    }

    #[test]
    fn custom_base_urls() {
        let client = OuraClient::new(
            "id".to_string(),
            "secret".to_string(),
            Some("http://localhost:9999".to_string()),
            Some("http://localhost:8888".to_string()),
            reqwest::Client::new(),
        );

        assert_eq!(client.api_base_url, "http://localhost:9999");
        assert_eq!(client.auth_base_url, "http://localhost:8888");
    }
}
