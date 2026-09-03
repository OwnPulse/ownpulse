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

### Why not the `__Host-` cookie prefix

`__Host-` would stop a sibling subdomain from setting a `Domain`-scoped
cookie of the same name that the browser then sends to us — a session
fixation vector, since the server cannot tell an injected cookie from its
own. The prefix requires `Path=/`, but the refresh cookie is deliberately
scoped to `Path=/api/v1/auth`, and the API shares an origin with the web
app: widening the path would attach the refresh token to every request for
every page and asset, multiplying the proxy and access logs it appears in.

Rather than trade one exposure for the other, the server refuses to guess.
`POST /auth/refresh` rejects a request presenting more than one *live*
`refresh_token` cookie (401, logged) — validity rather than count, so a
stale duplicate the user cannot clear themselves doesn't lock them out —
and `POST /auth/logout` revokes the token family of every cookie
presented. The shared cookie reader fails closed the same way for the
OAuth CSRF cookies.

**What this does not cover.** An injected cookie arriving in a browser
with no session of its own is the only one presented, so it is accepted
and establishes a session as the attacker's user. Defending that needs a
cookie a sibling cannot set at all. Two routes to it, both open:
`access_token` is already `Path=/`, so it can carry the `__Host-` prefix
for free; and the Google login callback still authenticates its CSRF
`state` against a cookie rather than the server-side `oauth_states` table
the Calendar connect flow uses. Revisit the prefix for `refresh_token`
itself if the API ever moves to its own origin, where `Path=/` costs
nothing.

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
2. **Back up your age private key.** Store it offline (USB drive, password manager). If you lose it, your encrypted backups are unrecoverable.
3. **Encrypt backups.** Use the provided backup script which encrypts with your age public key before uploading.
4. **Use a VPN for admin access.** Tailscale is recommended. Do not expose SSH or kubectl to the public internet.

## Roadmap

- Column-level encryption for genetic and lab data (at-rest encryption beyond volume-level).
- Row-level security (RLS) in Postgres for multi-tenant cooperative scenarios.
- Automatic key rotation for `ENCRYPTION_KEY` with re-encryption of existing tokens.
- Encrypted data exports (age-encrypted export archives).
- Audit logging for all data access and administrative actions.
- NetworkPolicy enforcement via kube-router deployment.
