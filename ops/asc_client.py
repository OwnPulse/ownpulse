#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) OwnPulse Contributors
"""App Store Connect API client for iOS crash diagnosis (Phase 1)."""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable

try:
    from cryptography.hazmat.primitives import hashes, serialization
    from cryptography.hazmat.primitives.asymmetric import ec
    from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature
except ImportError as exc:  # pragma: no cover
    raise ImportError(
        "The 'cryptography' package is required.\n"
        "Install it with: pip3 install cryptography"
    ) from exc


ASC_BASE_URL = "https://api.appstoreconnect.apple.com"
ASC_HOST = "api.appstoreconnect.apple.com"
ASC_AUDIENCE = "appstoreconnect-v1"
JWT_EXPIRY_SECONDS = 20 * 60


# --------------------------------------------------------------------------- #
# JWT
# --------------------------------------------------------------------------- #

def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def mint_jwt(key_id: str, issuer_id: str, key_pem: bytes) -> str:
    """Mint an ES256 JWT for App Store Connect (20 min expiry)."""
    try:
        private_key = serialization.load_pem_private_key(key_pem, password=None)
    except Exception as exc:
        raise ValueError(f"Failed to load private key: {exc}") from exc

    if not isinstance(private_key, ec.EllipticCurvePrivateKey):
        raise ValueError("Expected an EC P-256 private key (ES256).")

    header = {"alg": "ES256", "kid": key_id, "typ": "JWT"}
    now = int(time.time())
    payload = {
        "iss": issuer_id,
        "iat": now,
        "exp": now + JWT_EXPIRY_SECONDS,
        "aud": ASC_AUDIENCE,
    }

    header_b64 = _b64url(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    payload_b64 = _b64url(json.dumps(payload, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{header_b64}.{payload_b64}".encode("ascii")

    der_sig = private_key.sign(signing_input, ec.ECDSA(hashes.SHA256()))
    r, s = decode_dss_signature(der_sig)
    raw_sig = r.to_bytes(32, "big") + s.to_bytes(32, "big")
    sig_b64 = _b64url(raw_sig)

    return f"{header_b64}.{payload_b64}.{sig_b64}"


# --------------------------------------------------------------------------- #
# Credentials
# --------------------------------------------------------------------------- #

@dataclass(frozen=True)
class Credentials:
    key_id: str
    issuer_id: str
    app_id: str
    key_pem: bytes


_SOPS_REL_PATH = "secrets/ios/appstore-connect.sops.yaml"


def load_credentials(
    sops_path: Path | None = None,
    *,
    key_id: str | None = None,
    issuer_id: str | None = None,
    app_id: str | None = None,
    key_pem: bytes | None = None,
    _runner: Callable[..., subprocess.CompletedProcess] = subprocess.run,
) -> Credentials:
    """Resolve App Store Connect credentials.

    If `sops_path` is given, decrypt it in-process and parse the YAML.
    Otherwise expect all four kwargs (key_id, issuer_id, app_id, key_pem).
    The PEM never touches disk.
    """
    if sops_path is not None:
        try:
            import yaml  # type: ignore
        except ImportError as exc:
            raise ImportError(
                "The 'pyyaml' package is required to decode SOPS YAML.\n"
                "Install it with: pip3 install pyyaml"
            ) from exc

        if not sops_path.is_file():
            raise FileNotFoundError(f"SOPS file not found: {sops_path}")

        try:
            proc = _runner(
                ["sops", "-d", str(sops_path)],
                capture_output=True,
                check=False,
            )
        except FileNotFoundError as exc:
            raise RuntimeError(
                "'sops' not found in PATH; install with: "
                "brew install sops (or your platform equivalent)"
            ) from exc
        if proc.returncode != 0:
            stderr = (proc.stderr or b"").decode("utf-8", errors="replace")
            raise RuntimeError(f"sops -d failed (exit {proc.returncode}): {stderr.strip()}")

        doc = yaml.safe_load(proc.stdout) or {}
        try:
            pem = doc["key_pem"]
            if isinstance(pem, str):
                pem = pem.encode("utf-8")
            return Credentials(
                key_id=str(doc["key_id"]),
                issuer_id=str(doc["issuer_id"]),
                app_id=str(doc["app_id"]),
                key_pem=pem,
            )
        except KeyError as exc:
            raise ValueError(f"SOPS file missing required field: {exc}") from exc

    missing = [
        name for name, val in (
            ("key_id", key_id),
            ("issuer_id", issuer_id),
            ("app_id", app_id),
            ("key_pem", key_pem),
        ) if not val
    ]
    if missing:
        raise ValueError(f"Missing credential fields: {', '.join(missing)}")

    assert key_id and issuer_id and app_id and key_pem  # for type-checker
    return Credentials(
        key_id=key_id,
        issuer_id=issuer_id,
        app_id=app_id,
        key_pem=key_pem if isinstance(key_pem, bytes) else key_pem.encode("utf-8"),
    )


# --------------------------------------------------------------------------- #
# HTTP
# --------------------------------------------------------------------------- #

class ASCError(RuntimeError):
    def __init__(self, status: int, body: str, url: str) -> None:
        super().__init__(f"ASC {status} on {url}: {body[:500]}")
        self.status = status
        self.body = body
        self.url = url


def _http_get(url: str, headers: dict[str, str]) -> str:
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise ASCError(exc.code, body, url) from exc


def _request(token: str, url: str) -> dict[str, Any]:
    body = _http_get(url, {
        "Authorization": f"Bearer {token}",
        "Accept": "application/json",
    })
    return json.loads(body) if body else {}


def _assert_asc_host(url: str) -> None:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" or parsed.hostname != ASC_HOST:
        raise ASCError(
            0,
            f"Refusing to follow off-host URL: {url}",
            url,
        )


# --------------------------------------------------------------------------- #
# Public client functions
# --------------------------------------------------------------------------- #

def list_builds(
    token: str,
    app_id: str,
    since: datetime | None = None,
) -> list[dict]:
    """List builds for an app, newest first, optionally filtered by upload date."""
    url: str | None = (
        f"{ASC_BASE_URL}/v1/builds"
        f"?filter%5Bapp%5D={urllib.parse.quote(app_id)}"
        f"&sort=-uploadedDate"
        f"&limit=200"
    )
    builds: list[dict] = []
    while url:
        _assert_asc_host(url)
        payload = _request(token, url)
        builds.extend(payload.get("data", []))
        url = (payload.get("links") or {}).get("next")

    if since is not None:
        if since.tzinfo is None:
            since = since.replace(tzinfo=timezone.utc)
        filtered: list[dict] = []
        for b in builds:
            uploaded = (b.get("attributes") or {}).get("uploadedDate")
            if not uploaded:
                continue
            try:
                ts = datetime.fromisoformat(uploaded.replace("Z", "+00:00"))
            except ValueError:
                continue
            if ts >= since:
                filtered.append(b)
        return filtered

    return builds


# Field mapping is provisional — Apple's perfPowerMetrics crash schema is not formally documented. Adjust when we see real responses.
def _normalize_diagnostic(raw: dict, *, build_id: str) -> dict:
    attrs = raw.get("attributes") or {}
    return {
        "build_id": build_id,
        "signature": attrs.get("signature") or attrs.get("symbol") or raw.get("id"),
        "signal": attrs.get("signal") or attrs.get("exceptionType"),
        "stack": attrs.get("stack") or attrs.get("callStack") or attrs.get("symbols"),
        "os_version": attrs.get("osVersion") or attrs.get("platformVersion"),
        "device": attrs.get("deviceModel") or attrs.get("device"),
        "count": attrs.get("count") or attrs.get("occurrences"),
        "first_seen": attrs.get("firstSeen") or attrs.get("startDate"),
        "last_seen": attrs.get("lastSeen") or attrs.get("endDate"),
        "raw": raw,
    }


def crash_feedback(token: str, build_id: str) -> list[dict]:
    """Fetch crash signatures + tester notes for a build."""
    results: list[dict] = []

    metrics_url = f"{ASC_BASE_URL}/v1/builds/{urllib.parse.quote(build_id)}/perfPowerMetrics"
    try:
        metrics = _request(token, metrics_url)
        for row in metrics.get("data", []) or []:
            results.append(_normalize_diagnostic(row, build_id=build_id))
    except ASCError as exc:
        if exc.status != 404:
            raise

    loc_url = f"{ASC_BASE_URL}/v1/builds/{urllib.parse.quote(build_id)}/betaBuildLocalizations"
    try:
        locs = _request(token, loc_url)
        for row in locs.get("data", []) or []:
            attrs = row.get("attributes") or {}
            whats_new = attrs.get("whatsNew")
            if whats_new:
                results.append({
                    "build_id": build_id,
                    "signature": None,
                    "signal": None,
                    "stack": None,
                    "os_version": None,
                    "device": None,
                    "count": None,
                    "first_seen": None,
                    "last_seen": None,
                    "tester_notes": whats_new,
                    "raw": row,
                })
    except ASCError as exc:
        if exc.status != 404:
            raise

    return results


def download_dsyms(token: str, build_id: str, dest: Path) -> Path | None:
    """Download dSYM bundle for a build. Phase 3 — not yet implemented."""
    raise NotImplementedError("download_dsyms is a Phase 3 deliverable.")


# --------------------------------------------------------------------------- #
# Diagnose orchestration
# --------------------------------------------------------------------------- #

_DURATION_RE = re.compile(r"(\d+)([smhdw])")
_UNIT_SECONDS = {"s": 1, "m": 60, "h": 3600, "d": 86400, "w": 604800}


def _parse_since(spec: str) -> datetime:
    spec = spec.strip()
    m = _DURATION_RE.fullmatch(spec)
    if m:
        n, unit = int(m.group(1)), m.group(2)
        return datetime.now(timezone.utc) - timedelta(seconds=n * _UNIT_SECONDS[unit])
    try:
        ts = datetime.fromisoformat(spec.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"Invalid --since '{spec}'. Use e.g. 24h, 7d, or ISO 8601.") from exc
    if ts.tzinfo is None:
        ts = ts.replace(tzinfo=timezone.utc)
    return ts


def _resolve_credentials_from_cli(args: argparse.Namespace) -> Credentials:
    env_key_id = os.environ.get("ASC_KEY_ID")
    env_issuer = os.environ.get("ASC_ISSUER_ID")
    env_app_id = os.environ.get("ASC_APP_ID")
    env_pem = os.environ.get("ASC_KEY_PEM")

    if env_key_id and env_issuer and env_app_id and env_pem:
        return load_credentials(
            key_id=env_key_id,
            issuer_id=env_issuer,
            app_id=env_app_id,
            key_pem=env_pem.encode("utf-8"),
        )

    sops_path: Path | None = None
    explicit = getattr(args, "sops_path", None)
    if explicit:
        sops_path = Path(explicit)
    else:
        infra = os.environ.get("OWNPULSE_INFRA_PATH")
        if infra:
            sops_path = Path(infra) / _SOPS_REL_PATH

    if sops_path is None:
        raise SystemExit(
            "error: credentials missing — set OWNPULSE_INFRA_PATH or "
            "ASC_KEY_ID/ASC_ISSUER_ID/ASC_APP_ID/ASC_KEY_PEM"
        )

    return load_credentials(sops_path)


def _cmd_diagnose(args: argparse.Namespace) -> int:
    creds = _resolve_credentials_from_cli(args)
    app_id = args.app_id or creds.app_id

    token = mint_jwt(creds.key_id, creds.issuer_id, creds.key_pem)

    since = _parse_since(args.since) if args.since else None
    builds = list_builds(token, app_id, since=since)

    if args.build:
        builds = [b for b in builds if (b.get("attributes") or {}).get("version") == args.build]

    if args.device:
        print(
            "warning: --device filter is Apple-side-only and limited in Phase 1.",
            file=sys.stderr,
        )

    all_entries: list[dict] = []
    for b in builds:
        bid = b.get("id")
        if not bid:
            continue
        try:
            all_entries.extend(crash_feedback(token, bid))
        except ASCError as exc:
            print(f"warning: crash_feedback({bid}) failed: {exc}", file=sys.stderr)

    if args.signal:
        needle = args.signal.lower()
        all_entries = [
            e for e in all_entries
            if (e.get("signal") or "").lower().find(needle) >= 0
        ]

    if args.json:
        json.dump(all_entries, sys.stdout, indent=2, default=str)
        sys.stdout.write("\n")
        return 0

    _render_table(all_entries)
    return 0


def _render_table(entries: list[dict]) -> None:
    if not entries:
        print("(no crash entries returned)")
        return

    import collections
    groups: dict[str, list[dict]] = collections.defaultdict(list)
    for e in entries:
        groups[e.get("signature") or "(no signature)"].append(e)

    cols = ("signature", "signal", "count", "last_seen", "os", "device")
    widths = {c: len(c) for c in cols}
    rows = []
    for sig, items in groups.items():
        first = items[0]
        row = {
            "signature": (sig or "")[:60],
            "signal": str(first.get("signal") or ""),
            "count": str(sum(int(i.get("count") or 0) for i in items) or len(items)),
            "last_seen": str(first.get("last_seen") or ""),
            "os": str(first.get("os_version") or ""),
            "device": str(first.get("device") or ""),
        }
        rows.append((row, items))
        for c in cols:
            widths[c] = max(widths[c], len(row[c]))

    def fmt(row: dict) -> str:
        return " | ".join(row[c].ljust(widths[c]) for c in cols)

    print(fmt({c: c for c in cols}))
    print("-+-".join("-" * widths[c] for c in cols))
    for row, items in rows:
        print(fmt(row))
        for item in items:
            stack = item.get("stack")
            if stack:
                if isinstance(stack, list):
                    for line in stack:
                        print(f"    {line}")
                else:
                    for line in str(stack).splitlines():
                        print(f"    {line}")
            notes = item.get("tester_notes")
            if notes:
                for line in str(notes).splitlines():
                    print(f"    [tester] {line}")
        print()


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #

def _add_diagnose_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--since", help="Duration (24h, 7d, 30d) or ISO 8601 timestamp.")
    parser.add_argument("--build", help="Filter to a specific build version.")
    parser.add_argument("--device", help="Filter by device id (Phase 1: warning only).")
    parser.add_argument("--signal", help="Client-side filter on signal name (e.g. SIGSEGV).")
    parser.add_argument("--app-id", help="Override app id from credentials.")
    parser.add_argument("--sops-path", help="Path to SOPS-encrypted credentials YAML.")
    parser.add_argument("--json", action="store_true", help="Emit raw JSON.")


def _cmd_list_builds(args: argparse.Namespace) -> int:
    creds = _resolve_credentials_from_cli(args)
    app_id = args.app_id or creds.app_id
    token = mint_jwt(creds.key_id, creds.issuer_id, creds.key_pem)
    since = _parse_since(args.since) if args.since else None
    builds = list_builds(token, app_id, since=since)
    json.dump(builds, sys.stdout, indent=2, default=str)
    sys.stdout.write("\n")
    return 0


def _cmd_crash_feedback(args: argparse.Namespace) -> int:
    creds = _resolve_credentials_from_cli(args)
    token = mint_jwt(creds.key_id, creds.issuer_id, creds.key_pem)
    entries = crash_feedback(token, args.build_id)
    json.dump(entries, sys.stdout, indent=2, default=str)
    sys.stdout.write("\n")
    return 0


_EPILOG = """\
Credentials are resolved in this order:
  1. Env vars: ASC_KEY_ID, ASC_ISSUER_ID, ASC_APP_ID, ASC_KEY_PEM
     (ASC_KEY_PEM holds the *contents* of the .p8 file, not a path).
  2. --sops-path <file>, or
     $OWNPULSE_INFRA_PATH/secrets/ios/appstore-connect.sops.yaml.

The decrypted PEM stays in memory only; never written to disk.
"""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="asc_client.py",
        description="App Store Connect API client (Phase 1 of crash diagnosis tooling).",
        epilog=_EPILOG,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p_diag = sub.add_parser(
        "diagnose",
        help="End-to-end: list builds, fetch crashes, render report.",
        epilog=_EPILOG,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    _add_diagnose_args(p_diag)
    p_diag.set_defaults(func=_cmd_diagnose)

    p_list = sub.add_parser("list-builds", help="List builds for an app (JSON).")
    p_list.add_argument("--app-id")
    p_list.add_argument("--since")
    p_list.add_argument("--sops-path")
    p_list.set_defaults(func=_cmd_list_builds)

    p_crash = sub.add_parser("crash-feedback", help="Fetch crash feedback for a build (JSON).")
    p_crash.add_argument("--build-id", required=True)
    p_crash.add_argument("--sops-path")
    p_crash.set_defaults(func=_cmd_crash_feedback)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except SystemExit:
        raise
    except (ValueError, FileNotFoundError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    except ASCError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 3
    except urllib.error.URLError as exc:
        print(f"error: network: {exc}", file=sys.stderr)
        return 4
    except RuntimeError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 5


if __name__ == "__main__":
    sys.exit(main())
