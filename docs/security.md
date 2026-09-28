# Security Model

This document describes how OwnPulse protects user data. It is written for users evaluating the platform and self-hosters deploying their own instance.

## Encryption in Transit

All public traffic is TLS-terminated via nginx-ingress with certificates issued by Let's Encrypt (cert-manager `ClusterIssuer`). HSTS is enforced with `max-age=31536000; includeSubDomains`.

Administrative access (SSH, kubectl, monitoring) goes through a Tailscale mesh VPN. The server firewall allows only ports 80, 443, and 41641 (Tailscale WireGuard). No other ports are exposed to the public internet.

## Encryption at Rest

- **DigitalOcean volume encryption** — block storage is encrypted at the infrastructure layer.
- **Integration tokens** — OAuth tokens for third-party services (Garmin, Oura, Google Calendar) are encrypted with AES-256-GCM before storage. Each token gets a unique random 96-bit nonce. The key is a 32-byte hex value set via `ENCRYPTION_KEY`.
- **Passwords** — hashed with bcrypt.
- **Refresh tokens** — stored as HMAC-SHA256 hashes, not plaintext.
- **Backups** — encrypted with age (asymmetric, X25519). The age public key is stored on the server; the private key is kept offline.

## Authentication

- **JWT access tokens** — HS256, 1-hour expiry by default (`JWT_EXPIRY_SECONDS`). Transmitted in the `Authorization: Bearer` header. Never stored in localStorage or cookies.
- **Refresh tokens** — httpOnly, Secure, SameSite=Lax cookies. 30-day expiry by default (`REFRESH_TOKEN_EXPIRY_SECONDS`). Rotated on each use. A rotated token stays presentable for a 60-second grace window (web tabs share one cookie and race their refreshes) and always resolves to the same successor; reuse after the window is treated as theft and revokes the whole token family. A daily background sweep deletes token rows expired more than seven days — the margin keeps expired-token reuse detection alive.
- **Google OAuth** — used for signup and login. The server validates the Google ID token and issues its own JWT. No Google tokens are stored beyond the initial exchange.
- **Rate limiting** — login/register and the other credential-bearing auth endpoints share a 10 req/min per-IP bucket. `/auth/refresh` and `/auth/logout` share a separate 30 req/min per-IP bucket: hourly token refreshes and multi-tab bursts are routine traffic, and logout is the immediate token-revocation path — neither should compete with login attempts for budget. Per-IP limits key on `X-Forwarded-For`, so the outermost ingress **must strip or overwrite client-supplied `X-Forwarded-*` headers** (k3s's bundled Traefik does by default); an ingress that passes them through lets clients spoof their rate-limit identity.

### The `__Host-` cookie prefix, and where it does not fit

Cookies are scoped per site, not per origin, so a sibling subdomain can
set a `Domain`-scoped cookie of one of our names that the browser then
sends to us — a session fixation vector, since the server cannot tell an
injected cookie from its own. The `__Host-` prefix closes it: browsers
refuse a prefixed cookie that carries a `Domain` attribute, so only this
host can set one.

Two cookies carry the prefix on an HTTPS origin: the access token
(`__Host-access_token`) and the Google login CSRF nonce
(`__Host-oauth_state`). Both are `Path=/`, which the prefix requires and
neither pays for — one is already sent on every request, the other lives
ten minutes and is not a credential.

The prefix also demands `Secure`, which a plain-HTTP origin cannot set, so
those origins fall back to the bare names and **lose the guarantee
entirely** — see the caveat under Google login state below.

The refresh cookie does not, and cannot without a trade: it is
deliberately scoped to `Path=/api/v1/auth`, and the API shares an origin
with the web app, so widening to `Path=/` would attach the refresh token
to every request for every page and asset, multiplying the proxy and
access logs it appears in.

Rather than trade one exposure for the other, the server refuses to guess.
`POST /auth/refresh` rejects a request presenting more than one *live*
`refresh_token` cookie (401, logged) — validity rather than count, so a
stale duplicate the user cannot clear themselves doesn't lock them out —
and `POST /auth/logout` revokes the token family of every cookie
presented. The shared cookie reader fails closed the same way for the
OAuth CSRF cookies.

**What this does not cover.** An injected `refresh_token` cookie arriving
in a browser with no session of its own is the only one presented, so it
is accepted and establishes a session as the attacker's user. Defending
that needs the prefix, which the refresh cookie cannot take without
widening its path. Revisit it if the API ever moves to its own origin,
where `Path=/` costs nothing.

### Google login state

The Google login redirect carries nothing but the CSRF nonce in cookies.
`GET /auth/google/login` writes a single-use `login_oauth_states` row
holding the platform, the invite code, and — for `?mode=link` — the user id
resolved from the access-token cookie *at initiation*; the callback reads
all three from that row. It additionally requires the `__Host-oauth_state`
cookie to match the `state` parameter, because the row alone cannot prove
the callback reached the browser that started the flow: this endpoint is
unauthenticated, so an attacker can run a real flow of their own and
navigate a victim to the callback with a genuine state. The cookie supplies
browser binding; the row supplies integrity for data the browser should not
be trusted to carry. Abandoned rows in both state tables are cleared by the
hourly sweep.

**The browser-binding half is HTTPS-only.** On a plain-HTTP `WEB_ORIGIN`
the cookie falls back to the bare `oauth_state` name, which a sibling
subdomain can set — restoring the full login-CSRF chain the prefix exists
to break. The server still refuses the flow unless the cookie matches, so
an attacker needs cookie-write access on a sibling host, but that is
exactly the attacker this defends against. The endpoint is not hard-blocked
on HTTP: a LAN-only self-host is a legitimate deployment, and `WEB_ORIGIN`
describes the public origin, so anyone terminating TLS at a proxy already
gets the secure path. **Serve the app over HTTPS.**

## Client Security

- **Web** — JWT is held in memory (Zustand store). It does not survive page reload; the refresh cookie re-issues it. No sensitive data in localStorage or sessionStorage.
- **iOS** — JWT and refresh token are stored in the iOS Keychain. Never in UserDefaults or other unprotected storage.

## Network Isolation

- **Firewall** — ufw on the droplet. Only 80 (HTTP redirect), 443 (TLS), and 41641 (Tailscale) are open.
- **Tailscale VPN** — all admin traffic (SSH, kubectl, monitoring dashboards) routes through Tailscale. The Mac mini CI runner has no public ports at all.
- **Kubernetes NetworkPolicies** — planned but not yet enforced. The current Flannel CNI does not enforce NetworkPolicy rules. Migration to kube-router (which supports NetworkPolicy on top of Flannel's VXLAN) is planned. When enabled, policies will restrict pod-to-pod traffic to only the necessary paths (e.g., API to Postgres, web to API).

## Secrets Management

- **SOPS + age** — infrastructure secrets are encrypted with SOPS using age keys. Two age keys exist: one for the server, one for the developer. Both must be present to decrypt. Encrypted files are committed to the infra repo.
- **Bitnami SealedSecrets** — Kubernetes secrets are encrypted client-side with the cluster's public key and committed as `SealedSecret` resources. Only the cluster can decrypt them.

## Data Export and Deletion

- **Streaming export** — users can export all their data at any time in JSON, CSV, or FHIR R4 format. Exports are streamed and never buffered in full, so they work at any data volume.
- **Cascading delete** — account deletion removes all associated records (health records, interventions, observations, check-ins, lab results, calendar data, genetic records, integration tokens, export jobs).
- **Consent revocation** — cooperative data sharing consent can be revoked at any time. Revocation takes effect immediately with no grace period.

## Self-Hoster Checklist

1. **Set real secrets.** The server refuses to start if `JWT_SECRET` or `ENCRYPTION_KEY` are left at their default values when `WEB_ORIGIN` is not localhost.
2. **Serve over HTTPS and set `WEB_ORIGIN` to the `https://` URL.** Several cookies are host-only (`__Host-` prefixed) only on an HTTPS origin, because the prefix requires `Secure`. On plain HTTP they fall back to names any sibling subdomain can set, which is what the Google login CSRF defense rests on. If you terminate TLS at a reverse proxy, `WEB_ORIGIN` must still be the public `https://` URL.
3. **Back up your age private key.** Store it offline (USB drive, password manager). If you lose it, your encrypted backups are unrecoverable.
4. **Encrypt backups.** Use the provided backup script which encrypts with your age public key before uploading.
5. **Use a VPN for admin access.** Tailscale is recommended. Do not expose SSH or kubectl to the public internet.

## Roadmap

- Column-level encryption for genetic and lab data (at-rest encryption beyond volume-level).
- Row-level security (RLS) in Postgres for multi-tenant cooperative scenarios.
- Automatic key rotation for `ENCRYPTION_KEY` with re-encryption of existing tokens.
- Encrypted data exports (age-encrypted export archives).
- Audit logging for all data access and administrative actions.
- NetworkPolicy enforcement via kube-router deployment.
