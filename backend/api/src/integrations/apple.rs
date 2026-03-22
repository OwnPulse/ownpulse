// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Apple Sign-In identity token verification.
//!
//! Fetches Apple's JWKS, selects the matching key, and validates the
//! JWT signature, issuer, audience, and expiry. JWKS responses are
//! cached in memory for one hour to avoid hitting Apple on every login.

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::sync::RwLock;

/// Verified claims extracted from an Apple identity token.
pub struct AppleUserInfo {
    pub sub: String,
    pub email: Option<String>,
}

/// A single JSON Web Key from Apple's JWKS endpoint.
#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    /// Algorithm — Apple always uses RS256.
    #[allow(dead_code)]
    alg: String,
    n: String,
    e: String,
}

/// The JSON Web Key Set response from Apple.
#[derive(Debug, Clone, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Claims we extract from the Apple id_token JWT.
#[derive(Debug, Deserialize, Serialize)]
struct AppleClaims {
    /// Subject — the stable, unique Apple user ID.
    sub: String,
    /// Email — present when user grants email access; may be a relay address.
    email: Option<String>,
    /// Expiry (Unix timestamp).
    exp: u64,
    /// Issuer — must be "https://appleid.apple.com".
    iss: String,
    /// Audience — must match our client ID.
    aud: String,
}

/// Cached JWKS with the timestamp it was fetched.
struct CachedJwks {
    jwks: JwkSet,
    fetched_at: std::time::Instant,
}

/// In-memory JWKS cache with a 1-hour TTL.
static JWKS_CACHE: LazyLock<RwLock<Option<CachedJwks>>> = LazyLock::new(|| RwLock::new(None));

/// TTL for the JWKS cache.
const JWKS_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// Fetch the JWKS from Apple (or return the cached version if still fresh).
async fn fetch_jwks(
    client: &reqwest::Client,
    jwks_url: &str,
) -> Result<JwkSet, String> {
    // Try read lock first — fast path.
    {
        let cache = JWKS_CACHE.read().await;
        if let Some(ref cached) = *cache
            && cached.fetched_at.elapsed() < JWKS_CACHE_TTL
        {
            return Ok(cached.jwks.clone());
        }
    }

    // Cache is stale or empty — fetch and update under write lock.
    let mut cache = JWKS_CACHE.write().await;

    // Double-check after acquiring write lock (another task may have refreshed).
    if let Some(ref cached) = *cache
        && cached.fetched_at.elapsed() < JWKS_CACHE_TTL
    {
        return Ok(cached.jwks.clone());
    }

    let response = client
        .get(jwks_url)
        .send()
        .await
        .map_err(|e| format!("JWKS fetch failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("JWKS endpoint returned {status}"));
    }

    let jwks: JwkSet = response
        .json()
        .await
        .map_err(|e| format!("failed to parse JWKS response: {e}"))?;

    *cache = Some(CachedJwks {
        jwks: jwks.clone(),
        fetched_at: std::time::Instant::now(),
    });

    Ok(jwks)
}

/// Fetch Apple's JWKS and verify the `id_token`, returning the user's `sub`
/// and optional `email`.
///
/// # Arguments
/// * `client`     — Shared reqwest client.
/// * `id_token`   — The raw JWT string from the Apple response.
/// * `client_id`  — Our Apple Service ID / app bundle ID; must match `aud`.
/// * `jwks_url`   — Apple JWKS endpoint URL (overridable for tests).
pub async fn verify_identity_token(
    client: &reqwest::Client,
    id_token: &str,
    client_id: &str,
    jwks_url: &str,
) -> Result<AppleUserInfo, String> {
    // Decode the header to find which key id (kid) signed the token.
    let header =
        decode_header(id_token).map_err(|e| format!("failed to decode token header: {e}"))?;
    let token_kid = header.kid.ok_or("id_token header is missing 'kid'")?;

    // Fetch Apple's public keys (cached).
    let jwks = fetch_jwks(client, jwks_url).await?;

    // Find the matching key.
    let jwk = jwks
        .keys
        .into_iter()
        .find(|k| k.kid == token_kid)
        .ok_or_else(|| format!("no matching key found for kid={token_kid}"))?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| format!("failed to build RSA decoding key: {e}"))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["https://appleid.apple.com"]);
    validation.set_audience(&[client_id]);
    // jsonwebtoken validates exp by default; leeway stays at 0.

    let token_data = decode::<AppleClaims>(id_token, &decoding_key, &validation)
        .map_err(|e| format!("token validation failed: {e}"))?;

    Ok(AppleUserInfo {
        sub: token_data.claims.sub,
        email: token_data.claims.email,
    })
}
