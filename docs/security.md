# Security Model

This document describes how OwnPulse protects user data. It is written for users evaluating the platform and self-hosters deploying their own instance.

## Encryption in Transit

All public traffic is TLS-terminated via nginx-ingress with certificates issued by Let's Encrypt (cert-manager `ClusterIssuer`). HSTS is enforced with `max-age=31536000; includeSubDomains`.

Administrative access (SSH, kubectl, monitoring) goes through a Tailscale mesh VPN. The server firewall allows only ports 80, 443, and 41641 (Tailscale WireGuard). No other ports are exposed to the public internet.

## Encryption at Rest

- **DigitalOcean volume encryption** — block storage is encrypted at the infrastructure layer.
- **Integration tokens** — OAuth tokens for third-party services (Garmin, Oura, Dexcom, Google Calendar) are encrypted with AES-256-GCM before storage. Each token gets a unique random 96-bit nonce. The key is a 32-byte hex value set via `ENCRYPTION_KEY`.
- **Passwords** — hashed with bcrypt.
- **Refresh tokens** — stored as HMAC-SHA256 hashes, not plaintext.
- **Backups** — encrypted with age (asymmetric, X25519). The age public key is stored on the server; the private key is kept offline.

## Authentication

- **JWT access tokens** — HS256, 1-hour expiry by default (`JWT_EXPIRY_SECONDS`). Transmitted in the `Authorization: Bearer` header. Never stored in localStorage or cookies.
- **Refresh tokens** — httpOnly, Secure, SameSite=Lax cookies. 30-day expiry by default (`REFRESH_TOKEN_EXPIRY_SECONDS`). Rotated on each use; old tokens are invalidated.
- **Google OAuth** — used for signup and login. The server validates the Google ID token and issues its own JWT. No Google tokens are stored beyond the initial exchange.
- **Rate limiting** — login and token refresh endpoints are rate-limited to 5 requests per 60 seconds per IP.

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
