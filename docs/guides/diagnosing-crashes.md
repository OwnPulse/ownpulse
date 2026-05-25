# Diagnosing iOS crashes

## TL;DR

```bash
opdev crashes diagnose --since 24h
```

That pulls recent crash reports the user submitted through Apple's
"Share with App Developers" dialog, dedup-groups them by signature, and
prints stack traces. Phase 2 will merge in our own backend's
`app_events` records.

This is implemented as a subcommand of `opdev`, the OwnPulse developer
CLI. See [ownpulse-dev](https://github.com/OwnPulse/ownpulse-dev) for
opdev installation. Once installed, `opdev crashes diagnose --since 24h`
is your one entry point — no Python or shell scripts required in this
repo.

## Who needs this

This tooling is for whoever publishes the iOS build to TestFlight or the
App Store. It is **not** required to self-host the OwnPulse backend or
web app — those run fine without any App Store Connect access.

If you self-host OwnPulse and distribute your own iOS build (your own
App Store Connect account or enterprise distribution), you'll need your
own ASC API key, app ID, and issuer ID. Substitute them everywhere this
guide references `ownpulse-infra/secrets/ios/`. opdev itself is not
specific to our distribution — it works with any ASC credentials.

## Setup (one-time)

1. Sign in at https://appstoreconnect.apple.com.
2. Go to **Users and Access -> Integrations -> App Store Connect API**.
3. Click **Generate API Key** (or **+** if you already have keys).
   - **Name**: `ownpulse-crash-diag` (or similar).
   - **Access**: **Developer** role is sufficient (read-only access to
     `/v1/builds`, `/v1/builds/{id}/perfPowerMetrics`, and beta build
     localizations).
4. Download the `.p8` private key file. **Apple only lets you download
   this once.** Save it somewhere safe immediately.
5. Note the **Key ID** (10 characters, shown in the keys list) and the
   **Issuer ID** (UUID, at the top of the Integrations page).
6. Note your **App ID** from the app's listing
   (App Store Connect -> Apps -> OwnPulse -> App Information -> Apple ID).

You now have four pieces of data:

| Name        | Where it came from                       |
|-------------|------------------------------------------|
| Key ID      | Keys list, "Key ID" column               |
| Issuer ID   | Integrations page header                 |
| `.p8` file  | Downloaded once when key was created     |
| App ID      | App Information -> Apple ID              |

## Where credentials live

Credentials are SOPS-encrypted in the `ownpulse-infra` repo at
`secrets/ios/appstore-connect.sops.yaml` (per the
[secrets policy](../../CLAUDE.md): "Secrets in SOPS + age only").

opdev decrypts in-process — the PEM never touches disk.

### Option A — SOPS (recommended)

Point `OWNPULSE_INFRA_PATH` at the sibling checkout:

```bash
export OWNPULSE_INFRA_PATH="$HOME/src/ownpulse/ownpulse-infra"
opdev crashes diagnose --since 24h
```

Or pass `--sops-path` explicitly:

```bash
opdev crashes diagnose --sops-path /path/to/appstore-connect.sops.yaml --since 24h
```

### Option B — env vars (one-off debugging)

opdev reads these env vars as overrides when SOPS is unavailable. All
four must be set together. **`ASC_KEY_PEM` is the contents of the `.p8`
file, not a path** — this keeps the key off disk in the local flow too.

```bash
export ASC_KEY_ID="ABCD123456"
export ASC_ISSUER_ID="69a6de70-..."
export ASC_APP_ID="1234567890"
export ASC_KEY_PEM="$(cat ~/Downloads/AuthKey_ABCD123456.p8)"
opdev crashes diagnose --since 24h
```

## Common queries

Recent 24 hours, all builds:

```bash
opdev crashes diagnose --since 24h
```

Specific build only:

```bash
opdev crashes diagnose --build 142
```

Specific signal across the last week:

```bash
opdev crashes diagnose --since 7d --signal SIGSEGV
```

Raw JSON for piping into `jq`:

```bash
opdev crashes diagnose --since 24h --json | jq '.[] | select(.signal=="SIGABRT")'
```

## Troubleshooting

**`(no crash entries returned)`** — either there are no crashes in the
window (good!), or `ASC_APP_ID` is pointing at the wrong app. Verify
with:

```bash
opdev crashes list-builds
```

and confirm the latest TestFlight build shows up.

**`error: ASC 401`** — JWT was rejected by App Store Connect. Most
common causes:
- Key has been revoked in App Store Connect.
- `ASC_ISSUER_ID` is wrong (it's a UUID, not the Key ID).
- The PEM in `ASC_KEY_PEM` doesn't match `ASC_KEY_ID`.

**`error: sops binary not found`** — install with `brew install sops`,
then re-run. Only needed when using the SOPS path.

**`error: opdev: command not found`** — install opdev from the
[ownpulse-dev](https://github.com/OwnPulse/ownpulse-dev) repo. See that
repo's README for installation instructions.

## Limitations

- **Per-build aggregates, not per-device.** Apple's
  `perfPowerMetrics` endpoint surfaces signatures grouped by build, not
  by device. The `--device` flag exists but only warns until Phase 2
  brings our own backend channel online.
- **Symbolication depends on dSYMs.** If the build wasn't uploaded with
  symbols (`include_symbols: true` in fastlane / Xcode "Upload
  Symbols"), Apple-side reports come back with addresses rather than
  symbol names. Phase 3 wires automatic dSYM upload.
- **~24h lag.** Apple's pipeline aggregates crash reports
  asynchronously. Very recent crashes may not appear yet.
- **Tester notes != stack traces.** The
  `betaBuildLocalizations` view includes any tester-supplied notes for
  the build, but Apple does not expose raw "feedback" text via the
  public API. The user-visible "Share with App Developers" attachments
  surface as anonymized crash signatures only.
