// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! MyChart / SMART-on-FHIR HTTP client.
//!
//! MyChart (Epic) and other Cerner/Epic-style portals expose patient data via
//! the SMART-on-FHIR standard: standard OAuth 2.0 authorization-code flow with
//! PKCE, then a FHIR R4 REST API. This module handles:
//! - Token exchange and refresh against a per-provider token endpoint
//! - Fetching FHIR `Observation` (laboratory) and `DiagnosticReport` resources
//! - Parsing those resources into the existing `lab_results` shape
//!
//! Lab data is health data: it is imported verbatim. We never validate,
//! filter, or judge marker names or values — out-of-range flagging is derived
//! purely from the provider-supplied reference range.
//!
//! Endpoints are parameterised (token endpoint + FHIR base URL) because the
//! SMART-on-FHIR issuer varies per healthcare provider and so that WireMock can
//! stand in for a real server in tests.

use std::net::IpAddr;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::models::lab_result::CreateLabResult;

/// Source label written to `lab_results.source` for MyChart imports.
pub const SOURCE: &str = "mychart";

/// Validate a client-supplied SMART-on-FHIR provider URL before the server
/// makes any outbound request to it.
///
/// The `token_endpoint` and `fhir_base_url` come from the API client, and the
/// server connects to them directly — so without validation this is an SSRF
/// vector (an authenticated user could point the server at internal services
/// or the cloud metadata endpoint). We require HTTPS and reject hosts that are
/// IP literals in private / loopback / link-local / unspecified ranges.
///
/// `allow_insecure` relaxes the checks for local development and tests, where
/// the FHIR server is a WireMock instance on `http://127.0.0.1:<port>`.
pub fn validate_provider_url(raw: &str, allow_insecure: bool) -> Result<(), String> {
    if allow_insecure {
        return Ok(());
    }

    let url = reqwest::Url::parse(raw).map_err(|_| "invalid provider URL".to_string())?;

    if url.scheme() != "https" {
        return Err("provider URL must use https".to_string());
    }

    let host = url.host_str().ok_or("provider URL has no host")?;
    // `host_str()` wraps IPv6 literals in brackets (e.g. `[::1]`); strip them
    // before attempting to parse as an IP address.
    let host_ip = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);

    // Block IP-literal hosts in non-routable / internal ranges. Hostnames are
    // allowed (real FHIR issuers use DNS names); DNS-rebinding hardening is a
    // deeper mitigation tracked separately.
    if let Ok(ip) = host_ip.parse::<IpAddr>() {
        let blocked = match ip {
            IpAddr::V4(v4) => {
                v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || v4.octets()[0] == 0
            }
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    // unique-local (fc00::/7) and link-local (fe80::/10)
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        };
        if blocked {
            return Err("provider URL host is not allowed".to_string());
        }
    }

    Ok(())
}

// ── OAuth 2.0 types ─────────────────────────────────────────────────────

/// Token response from a SMART-on-FHIR token endpoint.
#[derive(Debug, Deserialize)]
pub struct MyChartTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub token_type: Option<String>,
}

// ── FHIR R4 response types (subset) ─────────────────────────────────────
//
// We deserialize only the fields we map into `lab_results`. Unknown fields are
// ignored by serde, which keeps the parser resilient to provider variation.

/// A FHIR `Bundle` of resources (search result page).
#[derive(Debug, Deserialize)]
pub struct FhirBundle {
    #[serde(default)]
    pub entry: Vec<FhirBundleEntry>,
}

#[derive(Debug, Deserialize)]
pub struct FhirBundleEntry {
    pub resource: Option<FhirObservation>,
}

/// A FHIR `Observation` resource (laboratory result).
#[derive(Debug, Deserialize)]
pub struct FhirObservation {
    pub id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    pub code: Option<FhirCodeableConcept>,
    #[serde(rename = "effectiveDateTime")]
    pub effective_date_time: Option<String>,
    #[serde(rename = "issued")]
    pub issued: Option<String>,
    #[serde(rename = "valueQuantity")]
    pub value_quantity: Option<FhirQuantity>,
    #[serde(rename = "referenceRange", default)]
    pub reference_range: Vec<FhirReferenceRange>,
}

#[derive(Debug, Deserialize)]
pub struct FhirCodeableConcept {
    pub text: Option<String>,
    #[serde(default)]
    pub coding: Vec<FhirCoding>,
}

#[derive(Debug, Deserialize)]
pub struct FhirCoding {
    pub display: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FhirQuantity {
    pub value: Option<f64>,
    pub unit: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FhirReferenceRange {
    pub low: Option<FhirQuantity>,
    pub high: Option<FhirQuantity>,
}

impl FhirObservation {
    /// Best-effort human-readable marker name. Prefers `code.text`, then the
    /// first coding `display`, then the coding `code`. Returns `None` if the
    /// observation has no usable name — such a record is skipped, never
    /// invented.
    pub fn marker_name(&self) -> Option<String> {
        let code = self.code.as_ref()?;
        if let Some(text) = code.text.as_deref().filter(|t| !t.trim().is_empty()) {
            return Some(text.to_string());
        }
        code.coding.iter().find_map(|c| {
            c.display
                .as_deref()
                .filter(|d| !d.trim().is_empty())
                .or(c.code.as_deref())
                .map(|s| s.to_string())
        })
    }

    /// Parse the effective date (date portion of `effectiveDateTime`, falling
    /// back to `issued`).
    pub fn panel_date(&self) -> Option<NaiveDate> {
        let raw = self
            .effective_date_time
            .as_deref()
            .or(self.issued.as_deref())?;
        parse_fhir_date(raw)
    }

    /// Convert this observation into a `CreateLabResult`, or `None` if it lacks
    /// the minimum fields (a FHIR resource id, a numeric value, a marker name,
    /// and a date).
    ///
    /// `source_id` is set to the FHIR resource id so re-syncs are idempotent
    /// via the existing `lab_results` dedup unique index. The index is partial
    /// (`WHERE source_id IS NOT NULL`), so an id-less observation could not be
    /// deduplicated on re-sync — we drop it rather than risk duplicate rows.
    pub fn to_lab_result(&self) -> Option<CreateLabResult> {
        let source_id = self.id.clone().filter(|s| !s.trim().is_empty())?;
        let value = self.value_quantity.as_ref().and_then(|q| q.value)?;
        let marker = self.marker_name()?;
        let panel_date = self.panel_date()?;
        let unit = self
            .value_quantity
            .as_ref()
            .and_then(|q| q.unit.clone())
            .unwrap_or_default();

        let range = self.reference_range.first();
        let reference_low = range.and_then(|r| r.low.as_ref()).and_then(|q| q.value);
        let reference_high = range.and_then(|r| r.high.as_ref()).and_then(|q| q.value);

        Some(CreateLabResult {
            panel_date,
            lab_name: None,
            marker,
            value,
            unit,
            reference_low,
            reference_high,
            source: Some(SOURCE.to_string()),
            source_id: Some(source_id),
        })
    }
}

/// Parse a FHIR `dateTime` / `date` into a `NaiveDate`. FHIR dates may be
/// `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, or a full RFC3339 timestamp. We only need
/// the date portion.
fn parse_fhir_date(raw: &str) -> Option<NaiveDate> {
    let date_part = raw.split('T').next().unwrap_or(raw);
    if let Ok(d) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        return Some(d);
    }
    // `YYYY-MM` -> first of month; `YYYY` -> Jan 1.
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{date_part}-01"), "%Y-%m-%d") {
        return Some(d);
    }
    if let Ok(d) = NaiveDate::parse_from_str(&format!("{date_part}-01-01"), "%Y-%m-%d") {
        return Some(d);
    }
    None
}

/// Parse a FHIR `Bundle` of `Observation` resources into lab results,
/// skipping resources that lack the minimum fields. Never panics on partial
/// data — incomplete observations are simply omitted.
pub fn parse_observation_bundle(bundle: &FhirBundle) -> Vec<CreateLabResult> {
    bundle
        .entry
        .iter()
        .filter_map(|e| e.resource.as_ref())
        .filter_map(|obs| obs.to_lab_result())
        .collect()
}

// ── Client ──────────────────────────────────────────────────────────────

/// HTTP client for a SMART-on-FHIR provider.
///
/// `token_endpoint` and `fhir_base_url` are per-provider values discovered by
/// the iOS client during the SMART launch and supplied at connect time. They
/// are overridable for WireMock testing.
pub struct MyChartClient {
    pub client_id: String,
    pub token_endpoint: String,
    pub fhir_base_url: String,
    pub http: reqwest::Client,
}

impl MyChartClient {
    pub fn new(
        client_id: String,
        token_endpoint: String,
        fhir_base_url: String,
        http: reqwest::Client,
    ) -> Self {
        Self {
            client_id,
            token_endpoint,
            fhir_base_url,
            http,
        }
    }

    /// Exchange an authorization code (PKCE) for tokens.
    ///
    /// SMART-on-FHIR public clients authenticate with `client_id` + PKCE
    /// `code_verifier` — there is no client secret.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: &str,
    ) -> Result<MyChartTokenResponse, String> {
        let response = self
            .http
            .post(&self.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", &self.client_id),
                ("redirect_uri", redirect_uri),
                ("code", code),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| format!("MyChart token exchange request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(format!("MyChart token exchange returned {status}"));
        }

        response
            .json::<MyChartTokenResponse>()
            .await
            .map_err(|e| format!("failed to parse MyChart token response: {e}"))
    }

    /// Refresh an expired access token.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<MyChartTokenResponse, String> {
        let response = self
            .http
            .post(&self.token_endpoint)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &self.client_id),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| format!("MyChart token refresh request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(format!("MyChart token refresh returned {status}"));
        }

        response
            .json::<MyChartTokenResponse>()
            .await
            .map_err(|e| format!("failed to parse MyChart token refresh response: {e}"))
    }

    /// Fetch laboratory `Observation` resources for the authenticated patient.
    ///
    /// SMART-on-FHIR exposes the patient identity via the access token, so the
    /// standard `Observation?category=laboratory` search returns the patient's
    /// own results. We do not pass a patient id from the client.
    pub async fn get_lab_observations(&self, access_token: &str) -> Result<FhirBundle, String> {
        let url = format!("{}/Observation", self.fhir_base_url.trim_end_matches('/'));
        let response = self
            .http
            .get(&url)
            .bearer_auth(access_token)
            .query(&[("category", "laboratory")])
            .header("Accept", "application/fhir+json")
            .send()
            .await
            .map_err(|e| format!("MyChart Observation request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(format!("MyChart Observation fetch returned {status}"));
        }

        response
            .json::<FhirBundle>()
            .await
            .map_err(|e| format!("failed to parse MyChart Observation bundle: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_provider_url_allows_https_dns_host() {
        assert!(validate_provider_url("https://fhir.example.org/r4", false).is_ok());
        assert!(validate_provider_url("https://fhir.example.org/oauth2/token", false).is_ok());
    }

    #[test]
    fn validate_provider_url_rejects_non_https() {
        assert!(validate_provider_url("http://fhir.example.org/r4", false).is_err());
        assert!(validate_provider_url("ftp://fhir.example.org/r4", false).is_err());
    }

    #[test]
    fn validate_provider_url_rejects_internal_ip_literals() {
        // Loopback, private, link-local (cloud metadata), unspecified.
        assert!(validate_provider_url("https://127.0.0.1/r4", false).is_err());
        assert!(validate_provider_url("https://10.0.0.5/r4", false).is_err());
        assert!(validate_provider_url("https://192.168.1.1/r4", false).is_err());
        assert!(validate_provider_url("https://169.254.169.254/latest/meta-data", false).is_err());
        assert!(validate_provider_url("https://[::1]/r4", false).is_err());
        assert!(validate_provider_url("https://[fc00::1]/r4", false).is_err());
    }

    #[test]
    fn validate_provider_url_allow_insecure_bypasses_checks() {
        // Local/dev/test path: WireMock on http://127.0.0.1:<port>.
        assert!(validate_provider_url("http://127.0.0.1:8080/r4", true).is_ok());
    }

    #[test]
    fn parses_full_rfc3339_date() {
        assert_eq!(
            parse_fhir_date("2026-03-28T08:30:00Z"),
            NaiveDate::from_ymd_opt(2026, 3, 28)
        );
    }

    #[test]
    fn parses_year_month_and_year_only() {
        assert_eq!(
            parse_fhir_date("2026-03"),
            NaiveDate::from_ymd_opt(2026, 3, 1)
        );
        assert_eq!(parse_fhir_date("2026"), NaiveDate::from_ymd_opt(2026, 1, 1));
    }

    #[test]
    fn marker_name_prefers_text_then_display_then_code() {
        let obs = FhirObservation {
            id: Some("o1".into()),
            status: Some("final".into()),
            code: Some(FhirCodeableConcept {
                text: Some("Hemoglobin A1c".into()),
                coding: vec![FhirCoding {
                    display: Some("HbA1c".into()),
                    code: Some("4548-4".into()),
                }],
            }),
            effective_date_time: Some("2026-03-28".into()),
            issued: None,
            value_quantity: Some(FhirQuantity {
                value: Some(5.4),
                unit: Some("%".into()),
            }),
            reference_range: vec![],
        };
        assert_eq!(obs.marker_name().as_deref(), Some("Hemoglobin A1c"));
    }

    #[test]
    fn to_lab_result_maps_value_unit_and_reference_range() {
        let obs = FhirObservation {
            id: Some("obs-123".into()),
            status: Some("final".into()),
            code: Some(FhirCodeableConcept {
                text: Some("Glucose".into()),
                coding: vec![],
            }),
            effective_date_time: Some("2026-03-28T08:30:00Z".into()),
            issued: None,
            value_quantity: Some(FhirQuantity {
                value: Some(92.0),
                unit: Some("mg/dL".into()),
            }),
            reference_range: vec![FhirReferenceRange {
                low: Some(FhirQuantity {
                    value: Some(70.0),
                    unit: Some("mg/dL".into()),
                }),
                high: Some(FhirQuantity {
                    value: Some(99.0),
                    unit: Some("mg/dL".into()),
                }),
            }],
        };

        let lab = obs.to_lab_result().expect("should map");
        assert_eq!(lab.marker, "Glucose");
        assert_eq!(lab.value, 92.0);
        assert_eq!(lab.unit, "mg/dL");
        assert_eq!(lab.reference_low, Some(70.0));
        assert_eq!(lab.reference_high, Some(99.0));
        assert_eq!(lab.source.as_deref(), Some("mychart"));
        assert_eq!(lab.source_id.as_deref(), Some("obs-123"));
    }

    #[test]
    fn observation_without_value_is_skipped() {
        let obs = FhirObservation {
            id: Some("obs-no-value".into()),
            status: Some("final".into()),
            code: Some(FhirCodeableConcept {
                text: Some("Note".into()),
                coding: vec![],
            }),
            effective_date_time: Some("2026-03-28".into()),
            issued: None,
            value_quantity: None,
            reference_range: vec![],
        };
        assert!(obs.to_lab_result().is_none());
    }

    #[test]
    fn observation_without_id_is_skipped() {
        // No FHIR id => no source_id => cannot be deduplicated on re-sync, so
        // it must be dropped rather than risk duplicate rows.
        let obs = FhirObservation {
            id: None,
            status: Some("final".into()),
            code: Some(FhirCodeableConcept {
                text: Some("Glucose".into()),
                coding: vec![],
            }),
            effective_date_time: Some("2026-03-28".into()),
            issued: None,
            value_quantity: Some(FhirQuantity {
                value: Some(92.0),
                unit: Some("mg/dL".into()),
            }),
            reference_range: vec![],
        };
        assert!(obs.to_lab_result().is_none());
    }

    #[test]
    fn observation_without_name_is_skipped() {
        let obs = FhirObservation {
            id: Some("obs-no-name".into()),
            status: Some("final".into()),
            code: None,
            effective_date_time: Some("2026-03-28".into()),
            issued: None,
            value_quantity: Some(FhirQuantity {
                value: Some(1.0),
                unit: Some("x".into()),
            }),
            reference_range: vec![],
        };
        assert!(obs.to_lab_result().is_none());
    }

    #[test]
    fn parse_bundle_skips_incomplete_entries() {
        let bundle = FhirBundle {
            entry: vec![
                FhirBundleEntry {
                    resource: Some(FhirObservation {
                        id: Some("good".into()),
                        status: Some("final".into()),
                        code: Some(FhirCodeableConcept {
                            text: Some("LDL".into()),
                            coding: vec![],
                        }),
                        effective_date_time: Some("2026-03-28".into()),
                        issued: None,
                        value_quantity: Some(FhirQuantity {
                            value: Some(110.0),
                            unit: Some("mg/dL".into()),
                        }),
                        reference_range: vec![],
                    }),
                },
                FhirBundleEntry { resource: None },
                FhirBundleEntry {
                    resource: Some(FhirObservation {
                        id: Some("bad".into()),
                        status: Some("final".into()),
                        code: None,
                        effective_date_time: None,
                        issued: None,
                        value_quantity: None,
                        reference_range: vec![],
                    }),
                },
            ],
        };

        let results = parse_observation_bundle(&bundle);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].marker, "LDL");
    }
}
