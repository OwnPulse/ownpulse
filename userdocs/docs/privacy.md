# Privacy & Security

OwnPulse is designed so that your health data stays under your control. This page explains what that means in practice.

## Your data, your instance

All data is stored on infrastructure you control -- your own server, your own database. There is no central OwnPulse analytics server that collects or processes user data. Each OwnPulse instance is independent and self-contained.

## Encryption

- **In transit** -- all communication between clients (web browser, iOS app) and your OwnPulse backend uses TLS encryption.
- **At rest** -- integration tokens (OAuth credentials for connected services) are encrypted with AES-256-GCM before being stored in the database. Your health data resides in a PostgreSQL database -- on cooperative infrastructure in your chosen region, or on your own server if you self-host.

## Authentication

OwnPulse uses JWT (JSON Web Token) authentication with automatic refresh token rotation. You do not need to manage token rotation yourself -- it happens transparently during normal use.

- **iOS** -- tokens are stored in the system Keychain, protected by the device's hardware security.
- **Web** -- the JWT is held in memory and the refresh token is stored as an httpOnly cookie, inaccessible to JavaScript.
- **Rate limiting** -- login attempts are rate limited to 5 per minute per IP address to prevent brute-force attacks.

## No tracking

OwnPulse includes zero analytics, zero telemetry, and zero third-party tracking scripts. No data leaves your account unless you explicitly trigger an action like connecting an integration or exporting your dataset. There are no background phone-home calls, no usage metrics, and no crash reporting sent to external services.

## Data sharing

Cooperative data sharing is a planned future feature. When implemented, it will require separate, explicit, revocable consent per dataset. You will choose exactly what to share and with whom. Consent can be revoked at any time with immediate effect. Today, nothing is shared -- this feature does not exist yet.

## Account deletion

Account deletion is permanent and immediate. When you delete your account, the following data is removed:

- All health records (from all sources)
- All daily check-ins
- All interventions
- All observations
- All lab results
- All integration connections and tokens
- Export history and audit log entries

This cannot be undone. There is no grace period and no recovery mechanism. Export your data before deleting your account if you want to keep a copy.
