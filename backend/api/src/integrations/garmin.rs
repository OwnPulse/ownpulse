// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Garmin Health API HTTP client.
//!
//! Garmin uses OAuth 1.0a. This module handles:
//! - Request token acquisition (temporary credentials)
//! - Access token exchange
//! - Signed API requests for health data
//!
//! All endpoints accept a base URL parameter for WireMock compatibility.

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

// ── OAuth 1.0a types ────────────────────────────────────────────────────

/// Temporary request token returned by Garmin during the OAuth 1.0a flow.
#[derive(Debug, Clone)]
pub struct RequestToken {
    pub oauth_token: String,
    pub oauth_token_secret: String,
}

/// Access token returned after the user authorizes the application.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub oauth_token: String,
    pub oauth_token_secret: String,
}

// ── API response types ──────────────────────────────────────────────────

/// A single daily summary from `/wellness-api/rest/dailies`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GarminDailySummary {
    pub calendar_date: Option<String>,
    pub total_steps: Option<i64>,
    pub resting_heart_rate: Option<f64>,
    pub max_heart_rate: Option<i64>,
    pub average_stress_level: Option<f64>,
    pub body_battery_highest_value: Option<f64>,
    pub body_battery_lowest_value: Option<f64>,
    pub total_kilocalories: Option<f64>,
}

/// A single sleep record from `/wellness-api/rest/sleeps`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GarminSleep {
    pub calendar_date: Option<String>,
    pub sleep_start_timestamp_gmt: Option<i64>,
    pub sleep_end_timestamp_gmt: Option<i64>,
    pub deep_sleep_seconds: Option<i64>,
    pub light_sleep_seconds: Option<i64>,
    pub rem_sleep_seconds: Option<i64>,
    pub awake_sleep_seconds: Option<i64>,
    pub overall_score: Option<f64>,
}

/// A single HRV record from `/wellness-api/rest/hrv`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GarminHrv {
    pub calendar_date: Option<String>,
    pub weekly_avg: Option<f64>,
    pub last_night: Option<f64>,
    pub status: Option<String>,
    pub start_timestamp_gmt: Option<i64>,
}

/// A single body composition record from `/wellness-api/rest/bodyComps`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GarminBodyComp {
    pub calendar_date: Option<String>,
    pub weight: Option<f64>,
    pub bmi: Option<f64>,
    pub body_fat: Option<f64>,
    pub muscle_mass: Option<f64>,
}

// ── OAuth 1.0a signature generation ─────────────────────────────────────

/// Percent-encode a string according to RFC 5849 Section 3.6.
fn percent_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Generate a random nonce for OAuth 1.0a requests.
fn generate_nonce() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}

/// Build the OAuth 1.0a Authorization header.
///
/// `method` — HTTP method (e.g. "GET", "POST").
/// `url` — full request URL (without query params for signature base).
/// `consumer_key` / `consumer_secret` — application credentials.
/// `token` / `token_secret` — user credentials (empty strings for request token step).
/// `extra_params` — additional query parameters to include in the signature.
fn build_oauth_header(
    method: &str,
    url: &str,
    consumer_key: &str,
    consumer_secret: &str,
    token: &str,
    token_secret: &str,
    extra_params: &[(&str, &str)],
) -> String {
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = generate_nonce();

    let mut params: Vec<(String, String)> = vec![
        ("oauth_consumer_key".to_string(), consumer_key.to_string()),
        ("oauth_nonce".to_string(), nonce.clone()),
        (
            "oauth_signature_method".to_string(),
            "HMAC-SHA1".to_string(),
        ),
        ("oauth_timestamp".to_string(), timestamp.clone()),
        ("oauth_version".to_string(), "1.0".to_string()),
    ];

    if !token.is_empty() {
        params.push(("oauth_token".to_string(), token.to_string()));
    }

    for (k, v) in extra_params {
        params.push((k.to_string(), v.to_string()));
    }

    params.sort();

    let param_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let base_string = format!(
        "{}&{}&{}",
        method.to_uppercase(),
        percent_encode(url),
        percent_encode(&param_string)
    );

    let signing_key = format!(
        "{}&{}",
        percent_encode(consumer_secret),
        percent_encode(token_secret)
    );

    let mut mac =
        HmacSha1::new_from_slice(signing_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(base_string.as_bytes());
    let signature = base64_encode(&mac.finalize().into_bytes());

    let mut header_params = vec![
        ("oauth_consumer_key", consumer_key.to_string()),
        ("oauth_nonce", nonce),
        ("oauth_signature", signature),
        ("oauth_signature_method", "HMAC-SHA1".to_string()),
        ("oauth_timestamp", timestamp),
        ("oauth_version", "1.0".to_string()),
    ];

    if !token.is_empty() {
        header_params.push(("oauth_token", token.to_string()));
    }

    let header_string: String = header_params
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join(", ");

    format!("OAuth {header_string}")
}

/// Base64-encode bytes (standard alphabet, with padding).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ── Client ──────────────────────────────────────────────────────────────

/// HTTP client for the Garmin Health API.
///
/// `base_url` can be overridden for WireMock testing; defaults to
/// `https://apis.garmin.com` in production.
pub struct GarminClient {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub base_url: String,
    pub http: reqwest::Client,
}

impl GarminClient {
    pub fn new(
        consumer_key: String,
        consumer_secret: String,
        base_url: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            consumer_key,
            consumer_secret,
            base_url: base_url.unwrap_or_else(|| "https://apis.garmin.com".to_string()),
            http,
        }
    }

    /// Step 1 of OAuth 1.0a: get a request token.
    pub async fn get_request_token(
        &self,
        callback_url: &str,
    ) -> Result<RequestToken, String> {
        let url = format!("{}/oauth-service/oauth/request_token", self.base_url);

        let header = build_oauth_header(
            "POST",
            &url,
            &self.consumer_key,
            &self.consumer_secret,
            "",
            "",
            &[("oauth_callback", callback_url)],
        );

        let response = self
            .http
            .post(&url)
            .header("Authorization", &header)
            .query(&[("oauth_callback", callback_url)])
            .send()
            .await
            .map_err(|e| format!("request token request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("request token returned {status}: {body}"));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("failed to read request token response: {e}"))?;

        parse_oauth_response(&body)
            .map(|(token, secret)| RequestToken {
                oauth_token: token,
                oauth_token_secret: secret,
            })
            .ok_or_else(|| "failed to parse request token response".to_string())
    }

    /// Build the authorization URL for step 2 of OAuth 1.0a.
    pub fn authorization_url(&self, request_token: &str) -> String {
        format!(
            "https://connect.garmin.com/oauthConfirm?oauth_token={}",
            percent_encode(request_token)
        )
    }

    /// Step 3 of OAuth 1.0a: exchange the request token + verifier for an access token.
    pub async fn get_access_token(
        &self,
        request_token: &str,
        request_token_secret: &str,
        oauth_verifier: &str,
    ) -> Result<AccessToken, String> {
        let url = format!("{}/oauth-service/oauth/access_token", self.base_url);

        let header = build_oauth_header(
            "POST",
            &url,
            &self.consumer_key,
            &self.consumer_secret,
            request_token,
            request_token_secret,
            &[("oauth_verifier", oauth_verifier)],
        );

        let response = self
            .http
            .post(&url)
            .header("Authorization", &header)
            .query(&[("oauth_verifier", oauth_verifier)])
            .send()
            .await
            .map_err(|e| format!("access token request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("access token returned {status}: {body}"));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("failed to read access token response: {e}"))?;

        parse_oauth_response(&body)
            .map(|(token, secret)| AccessToken {
                oauth_token: token,
                oauth_token_secret: secret,
            })
            .ok_or_else(|| "failed to parse access token response".to_string())
    }

    /// Fetch daily summaries for a date range.
    pub async fn get_daily_summary(
        &self,
        token: &AccessToken,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<GarminDailySummary>, String> {
        let url = format!("{}/wellness-api/rest/dailies", self.base_url);
        self.signed_get(&url, token, &[("calendarDate", start_date), ("calendarDateEnd", end_date)])
            .await
    }

    /// Fetch sleep data for a date range.
    pub async fn get_sleep(
        &self,
        token: &AccessToken,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<GarminSleep>, String> {
        let url = format!("{}/wellness-api/rest/sleeps", self.base_url);
        self.signed_get(&url, token, &[("calendarDate", start_date), ("calendarDateEnd", end_date)])
            .await
    }

    /// Fetch HRV data for a date range.
    pub async fn get_hrv(
        &self,
        token: &AccessToken,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<GarminHrv>, String> {
        let url = format!("{}/wellness-api/rest/hrv", self.base_url);
        self.signed_get(&url, token, &[("calendarDate", start_date), ("calendarDateEnd", end_date)])
            .await
    }

    /// Fetch body composition data for a date range.
    pub async fn get_body_comp(
        &self,
        token: &AccessToken,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<GarminBodyComp>, String> {
        let url = format!("{}/wellness-api/rest/bodyComps", self.base_url);
        self.signed_get(&url, token, &[("startDate", start_date), ("endDate", end_date)])
            .await
    }

    /// Make a signed GET request and deserialize the JSON response.
    async fn signed_get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        token: &AccessToken,
        query_params: &[(&str, &str)],
    ) -> Result<T, String> {
        let header = build_oauth_header(
            "GET",
            url,
            &self.consumer_key,
            &self.consumer_secret,
            &token.oauth_token,
            &token.oauth_token_secret,
            query_params,
        );

        let response = self
            .http
            .get(url)
            .header("Authorization", &header)
            .query(query_params)
            .send()
            .await
            .map_err(|e| format!("Garmin API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unreadable body".into());
            return Err(format!("Garmin API returned {status}: {body}"));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| format!("failed to parse Garmin API response: {e}"))
    }
}

/// Parse an OAuth 1.0a response body (form-encoded) into (token, secret).
fn parse_oauth_response(body: &str) -> Option<(String, String)> {
    let mut token = None;
    let mut secret = None;
    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        match (parts.next(), parts.next()) {
            (Some("oauth_token"), Some(v)) => token = Some(v.to_string()),
            (Some("oauth_token_secret"), Some(v)) => secret = Some(v.to_string()),
            _ => {}
        }
    }
    token.zip(secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_preserves_unreserved() {
        assert_eq!(percent_encode("abcXYZ019"), "abcXYZ019");
        assert_eq!(percent_encode("a-b.c_d~e"), "a-b.c_d~e");
    }

    #[test]
    fn percent_encode_encodes_special() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("a=b&c"), "a%3Db%26c");
    }

    #[test]
    fn parse_oauth_response_valid() {
        let body = "oauth_token=abc123&oauth_token_secret=xyz789&oauth_callback_confirmed=true";
        let (token, secret) = parse_oauth_response(body).unwrap();
        assert_eq!(token, "abc123");
        assert_eq!(secret, "xyz789");
    }

    #[test]
    fn parse_oauth_response_missing_fields() {
        assert!(parse_oauth_response("oauth_token=abc123").is_none());
        assert!(parse_oauth_response("unrelated=value").is_none());
    }

    #[test]
    fn base64_encode_simple() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn build_oauth_header_contains_required_params() {
        let header = build_oauth_header(
            "GET",
            "https://example.com/api",
            "consumer_key",
            "consumer_secret",
            "token",
            "token_secret",
            &[],
        );
        assert!(header.starts_with("OAuth "));
        assert!(header.contains("oauth_consumer_key=\"consumer_key\""));
        assert!(header.contains("oauth_token=\"token\""));
        assert!(header.contains("oauth_signature_method=\"HMAC-SHA1\""));
        assert!(header.contains("oauth_version=\"1.0\""));
    }

    #[test]
    fn build_oauth_header_without_token() {
        let header = build_oauth_header(
            "POST",
            "https://example.com/api",
            "consumer_key",
            "consumer_secret",
            "",
            "",
            &[("oauth_callback", "https://example.com/callback")],
        );
        assert!(header.starts_with("OAuth "));
        assert!(!header.contains("oauth_token="));
    }
}
