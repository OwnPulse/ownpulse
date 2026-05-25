# Diagnosing iOS crashes

## TL;DR

```bash
ops/diag-crashes.sh --since 24h
```

That pulls recent crash reports the user submitted through Apple's
"Share with App Developers" dialog, dedup-groups them by signature, and
prints stack traces. Phase 2 will merge in our own backend's
`app_events` records.

## Who needs this

This tooling is for whoever publishes the iOS build to TestFlight or the
App Store. It is **not** required to self-host the OwnPulse backend or
web app — those run fine without any App Store Connect access.

If you self-host OwnPulse and distribute your own iOS build (your own
App Store Connect account or enterprise distribution), you'll need your
own ASC API key, app ID, and issuer ID. Substitute them everywhere this
guide references `ownpulse-infra/secrets/ios/`. The Python client and
shell shim are not specific to our distribution — they'll work with any
ASC credentials.

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

The script decrypts in-process — the PEM never touches disk.

### Option A — SOPS (recommended)

Point `OWNPULSE_INFRA_PATH` at the sibling checkout:

```bash
export OWNPULSE_INFRA_PATH="$HOME/src/ownpulse/ownpulse-infra"
ops/diag-crashes.sh --since 24h
```

Or pass `--sops-path` explicitly:

```bash
ops/diag-crashes.sh --sops-path /path/to/appstore-connect.sops.yaml --since 24h
```

### Option B — env vars (one-off debugging)

All four must be set together. **`ASC_KEY_PEM` is the contents of the
`.p8` file, not a path** — this keeps the key off disk in the local
flow too.

```bash
export ASC_KEY_ID="ABCD123456"
export ASC_ISSUER_ID="69a6de70-..."
export ASC_APP_ID="1234567890"
export ASC_KEY_PEM="$(cat ~/Downloads/AuthKey_ABCD123456.p8)"
ops/diag-crashes.sh --since 24h
```

## Common queries

Recent 24 hours, all builds:

```bash
ops/diag-crashes.sh --since 24h
```

Specific build only:

```bash
ops/diag-crashes.sh --build 142
```

Specific signal across the last week:

```bash
ops/diag-crashes.sh --since 7d --signal SIGSEGV
```

Raw JSON for piping into `jq`:

```bash
ops/diag-crashes.sh --since 24h --json | jq '.[] | select(.signal=="SIGABRT")'
```

## Troubleshooting

**`(no crash entries returned)`** — either there are no crashes in the
window (good!), or `ASC_APP_ID` is pointing at the wrong app. Verify
with:

```bash
python3 ops/asc_client.py list-builds
```

and confirm the latest TestFlight build shows up.

**`error: ASC 401`** — JWT was rejected. Most common causes:
- Key has been revoked in App Store Connect.
- `ASC_ISSUER_ID` is wrong (it's a UUID, not the Key ID).
- The PEM in `ASC_KEY_PEM` doesn't match `ASC_KEY_ID`.

**`error: 'sops' not found`** — install with `brew install sops`,
then re-run. Only needed when using the SOPS path.

**`ImportError: cryptography`** / **`pyyaml`** — install with
`pip3 install cryptography pyyaml`. The CLI requires `cryptography` for
ES256 signing and `pyyaml` to parse SOPS-decrypted YAML.

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
