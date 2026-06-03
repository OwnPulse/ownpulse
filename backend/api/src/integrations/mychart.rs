// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! MyChart / SMART-on-FHIR HTTP client.
//!
//! MyChart (Epic) and other Cerner/Epic-style portals expose patient data via
//! the SMART-on-FHIR standard: standard OAuth 2.0 authorization-code flow with
//! PKCE, then a FHIR R4 REST API. This module handles:
//! - Token exchange and refresh against a per-provider token endpoint
//! - Fetching FHIR `Observation` (laboratory) resources
//! - Parsing those resources into the existing `lab_results` shape
//!
//! Scope is laboratory `Observation` resources only. `DiagnosticReport`
//! (panel grouping) is intentionally out of scope for this iteration — the
//! referenced `Observation` resources are imported directly.
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

/// Return true if an IPv4 address is in a non-routable / internal range that
/// must never be reachable from a server-side SSRF.
fn is_blocked_v4(v4: std::net::Ipv4Addr) -> bool {
    v4.is_private()        // 10/8, 172.16/12, 192.168/16
        || v4.is_loopback()    // 127/8
        || v4.is_link_local()  // 169.254/16 (cloud metadata)
        || v4.is_unspecified() // 0.0.0.0
        || v4.is_broadcast()   // 255.255.255.255
        || v4.is_documentation()
        || v4.octets()[0] == 0 // 0/8
        || v4.octets()[0] >= 224 // multicast (224/4) + reserved (240/4)
        // Carrier-grade NAT 100.64/10 and shared/benchmark ranges.
        || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
}

/// Return true if an IPv6 address is internal. Canonicalises every IPv6 form
/// that can embed or route to an IPv4 address down to that IPv4 and re-checks
/// it against [`is_blocked_v4`] — otherwise `[::ffff:169.254.169.254]`,
/// `[64:ff9b::169.254.169.254]` (NAT64), or `[2002:a9fe:a9fe::]` (6to4) would
/// slip through.
fn is_blocked_v6(v6: std::net::Ipv6Addr) -> bool {
    let seg = v6.segments();

    // IPv4-mapped (`::ffff:a.b.c.d`) / IPv4-compatible: re-check embedded V4.
    if let Some(mapped) = v6.to_ipv4() {
        return is_blocked_v4(mapped);
    }

    // NAT64 well-known prefix 64:ff9b::/96 embeds the IPv4 in the low 32 bits.
    if seg[0] == 0x0064
        && seg[1] == 0xff9b
        && seg[2] == 0
        && seg[3] == 0
        && seg[4] == 0
        && seg[5] == 0
    {
        let embedded = std::net::Ipv4Addr::new(
            (seg[6] >> 8) as u8,
            (seg[6] & 0xff) as u8,
            (seg[7] >> 8) as u8,
            (seg[7] & 0xff) as u8,
        );
        return is_blocked_v4(embedded);
    }

    // 6to4 (2002::/16) embeds the IPv4 in the next 32 bits.
    if seg[0] == 0x2002 {
        let embedded = std::net::Ipv4Addr::new(
            (seg[1] >> 8) as u8,
            (seg[1] & 0xff) as u8,
            (seg[2] >> 8) as u8,
            (seg[2] & 0xff) as u8,
        );
        // 6to4 always routes via an IPv4 underlay; if that underlay is internal,
        // block it. Public 6to4 underlays remain reachable.
        return is_blocked_v4(embedded);
    }

    v6.is_loopback()        // ::1
        || v6.is_unspecified() // ::
        // unique-local fc00::/7
        || (seg[0] & 0xfe00) == 0xfc00
        // link-local fe80::/10
        || (seg[0] & 0xffc0) == 0xfe80
        // Teredo 2001::/32 (tunnels over IPv4 — treat as untrusted underlay)
        || (seg[0] == 0x2001 && seg[1] == 0x0000)
        // IPv6 documentation 2001:db8::/32
        || (seg[0] == 0x2001 && seg[1] == 0x0db8)
}

/// True if an IP address (V4 or V6, including IPv4-mapped V6) is internal.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

/// Validate a client-supplied SMART-on-FHIR provider URL before the server
/// makes any outbound request to it.
///
/// The `token_endpoint` and `fhir_base_url` come from the API client, and the
/// server connects to them directly — so without validation this is an SSRF
/// vector (an authenticated user could point the server at internal services
/// or the cloud metadata endpoint). Defenses, in order:
///
/// 1. Require `https`.
/// 2. Reject IP-literal hosts in internal ranges, canonicalising IPv4-mapped
///    IPv6 (`::ffff:169.254.169.254`) back to IPv4 first.
/// 3. Reject all-numeric / hex / octal host forms (`0x7f.0.0.1`,
///    `2130706433`) that resolvers would interpret as internal IPs but that
///    are not valid `IpAddr` literals.
///
/// This is host-string validation. DNS rebinding (a hostname that resolves to
/// an internal IP) is closed separately by [`build_client`], which validates
/// the *resolved* socket address at connect time.
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

    if let Ok(ip) = host_ip.parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            return Err("provider URL host is not allowed".to_string());
        }
        return Ok(());
    }

    // Not a valid IP literal. Reject host forms that look like obfuscated IPs
    // (decimal `2130706433`, hex `0x7f000001`, octal `0177.0.0.1`,
    // leading-zero octets). A legitimate FHIR hostname has at least one
    // non-numeric, non-`0x` label.
    if looks_like_numeric_host(host_ip) {
        return Err("provider URL host is not allowed".to_string());
    }

    Ok(())
}

/// Heuristic: does this host string look like an obfuscated numeric IP rather
/// than a DNS hostname? Catches all-decimal, hex (`0x..`), octal (leading
/// zero), and dotted-numeric forms that `IpAddr::parse` rejected but a libc
/// resolver may still accept as an integer-encoded address.
fn looks_like_numeric_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    // A real hostname always contains at least one alphabetic character in a
    // label that is not a `0x` hex prefix. If every label is purely numeric or
    // hex/octal, treat it as a numeric host.
    host.split('.').all(|label| {
        let l = label.trim();
        if l.is_empty() {
            return false;
        }
        let hex = l.strip_prefix("0x").or_else(|| l.strip_prefix("0X"));
        match hex {
            Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit()),
            None => l.bytes().all(|b| b.is_ascii_digit()),
        }
    })
}

/// Build a dedicated reqwest client for talking to a SMART-on-FHIR provider.
///
/// Two SSRF defenses beyond [`validate_provider_url`]:
/// - **No redirects**: SMART token/FHIR endpoints do not need cross-host
///   redirects, and following them would let a valid host bounce the server to
///   `169.254.169.254` / loopback after the URL passed validation.
/// - **Resolved-address filter**: a custom DNS resolver rejects any name that
///   resolves to an internal range, closing the DNS-rebinding hole where a
///   hostname passes string validation yet resolves to an internal IP.
///
/// `allow_insecure` (dev/tests only) drops the resolver so WireMock on
/// `127.0.0.1` is reachable; redirects stay disabled regardless.
pub fn build_client(allow_insecure: bool) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    if !allow_insecure {
        builder = builder.dns_resolver(std::sync::Arc::new(SsrfGuardResolver));
    }
    builder
        .build()
        .map_err(|e| format!("failed to build MyChart HTTP client: {e}"))
}

/// A reqwest DNS resolver that resolves names normally but rejects any address
/// in an internal range — closing the DNS-rebinding hole where a hostname
/// passes string validation yet resolves to `127.0.0.1` / `169.254.169.254`.
struct SsrfGuardResolver;

impl reqwest::dns::Resolve for SsrfGuardResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            // Resolve on a blocking thread via the std resolver.
            let addrs = tokio::task::spawn_blocking(move || {
                std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), 0u16))
                    .map(|it| it.collect::<Vec<_>>())
            })
            .await;

            let addrs = match addrs {
                Ok(Ok(addrs)) => addrs,
                _ => {
                    return Err(Box::<dyn std::error::Error + Send + Sync>::from(
                        "dns resolution failed",
                    ));
                }
            };

            if addrs.iter().any(|sa| is_blocked_ip(sa.ip())) {
                return Err(Box::<dyn std::error::Error + Send + Sync>::from(
                    "resolved address is not allowed",
                ));
            }

            let iter: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
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
    pub system: Option<String>,
    pub display: Option<String>,
    pub code: Option<String>,
}

/// Canonical LOINC code system URI.
const LOINC_SYSTEM: &str = "http://loinc.org";

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

    /// Extract the LOINC code from the observation's codings, if present.
    /// Preserved so the open-schema export carries the standard code, not just
    /// the provider's display text.
    pub fn loinc_code(&self) -> Option<String> {
        let code = self.code.as_ref()?;
        code.coding
            .iter()
            .find(|c| c.system.as_deref() == Some(LOINC_SYSTEM))
            .and_then(|c| c.code.as_deref())
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.to_string())
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
            loinc_code: self.loinc_code(),
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
    ///
    /// The response body is capped at [`MAX_RESPONSE_BYTES`] so a malicious or
    /// compromised provider cannot exhaust memory with a giant bundle.
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

        let body = read_capped_body(response).await?;
        serde_json::from_slice::<FhirBundle>(&body)
            .map_err(|e| format!("failed to parse MyChart Observation bundle: {e}"))
    }
}

/// Maximum FHIR response body the server will buffer (16 MiB). A FHIR lab
/// bundle for one patient is kilobytes; this only trips on abuse.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Read a response body into memory, aborting if it exceeds
/// [`MAX_RESPONSE_BYTES`]. Streams chunk-by-chunk so an oversized body is
/// rejected without first being fully buffered.
async fn read_capped_body(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("MyChart response read failed: {e}"))?
    {
        if buf.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err("MyChart response exceeded maximum allowed size".to_string());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
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
    fn validate_provider_url_rejects_ipv4_mapped_ipv6() {
        // IPv4-mapped / -compatible IPv6 must be canonicalised and blocked —
        // otherwise [::ffff:169.254.169.254] reaches cloud metadata.
        assert!(validate_provider_url("https://[::ffff:169.254.169.254]/x", false).is_err());
        assert!(validate_provider_url("https://[::ffff:127.0.0.1]/x", false).is_err());
        assert!(validate_provider_url("https://[::ffff:10.0.0.1]/x", false).is_err());
    }

    #[test]
    fn validate_provider_url_rejects_ipv6_embedded_and_tunnel_prefixes() {
        // NAT64 well-known prefix embedding cloud metadata / loopback.
        assert!(validate_provider_url("https://[64:ff9b::169.254.169.254]/x", false).is_err());
        assert!(validate_provider_url("https://[64:ff9b::127.0.0.1]/x", false).is_err());
        // 6to4 embedding a link-local IPv4 underlay (2002:a9fe:a9fe:: = 169.254.169.254).
        assert!(validate_provider_url("https://[2002:a9fe:a9fe::]/x", false).is_err());
        // Teredo and IPv6 documentation prefixes.
        assert!(validate_provider_url("https://[2001:0:abcd::1]/x", false).is_err());
        assert!(validate_provider_url("https://[2001:db8::1]/x", false).is_err());
    }

    #[test]
    fn validate_provider_url_rejects_alternate_ip_encodings() {
        // Decimal, hex, octal, and leading-zero host forms that a libc resolver
        // would treat as internal IPs but that are not valid IpAddr literals.
        assert!(validate_provider_url("https://2130706433/x", false).is_err()); // 127.0.0.1
        assert!(validate_provider_url("https://0x7f000001/x", false).is_err());
        assert!(validate_provider_url("https://0177.0.0.1/x", false).is_err());
        assert!(validate_provider_url("https://0x7f.0.0.1/x", false).is_err());
    }

    #[test]
    fn validate_provider_url_rejects_extra_internal_ranges() {
        assert!(validate_provider_url("https://100.64.0.1/x", false).is_err()); // CGNAT
        assert!(validate_provider_url("https://224.0.0.1/x", false).is_err()); // multicast
    }

    #[test]
    fn validate_provider_url_allows_legitimate_public_host() {
        assert!(
            validate_provider_url("https://fhir.epic.com/interconnect/api/FHIR/R4", false).is_ok()
        );
        assert!(validate_provider_url("https://8.8.8.8/r4", false).is_ok());
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
                    system: Some("http://loinc.org".into()),
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
                coding: vec![FhirCoding {
                    system: Some("http://loinc.org".into()),
                    display: Some("Glucose".into()),
                    code: Some("2339-0".into()),
                }],
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
        assert_eq!(lab.loinc_code.as_deref(), Some("2339-0"));
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
