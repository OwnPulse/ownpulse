# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) OwnPulse Contributors
from __future__ import annotations

import base64
import json
import subprocess
import sys
import time
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import asc_client  # noqa: E402
from asc_client import (  # noqa: E402
    ASCError,
    Credentials,
    _b64url,
    list_builds,
    load_credentials,
    mint_jwt,
)

from cryptography.hazmat.primitives import hashes, serialization  # noqa: E402
from cryptography.hazmat.primitives.asymmetric import ec  # noqa: E402
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature  # noqa: E402


def _b64url_decode(s: str) -> bytes:
    pad = "=" * (-len(s) % 4)
    return base64.urlsafe_b64decode(s + pad)


def _generate_key() -> tuple[bytes, ec.EllipticCurvePublicKey]:
    private_key = ec.generate_private_key(ec.SECP256R1())
    pem = private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    )
    return pem, private_key.public_key()


class TestB64Url(unittest.TestCase):
    def test_round_trip(self):
        for payload in (b"", b"a", b"hello", b"\x00\xff\x10", b"x" * 100):
            encoded = _b64url(payload)
            self.assertNotIn("=", encoded)
            self.assertEqual(_b64url_decode(encoded), payload)

    def test_no_padding(self):
        self.assertEqual(_b64url(b"a"), "YQ")
        self.assertEqual(_b64url(b"ab"), "YWI")
        self.assertEqual(_b64url(b"abc"), "YWJj")


class TestMintJwt(unittest.TestCase):
    def test_structure_and_signature(self):
        pem, public_key = _generate_key()
        before = int(time.time())
        token = mint_jwt("ABCD123456", "issuer-uuid", pem)
        after = int(time.time())

        parts = token.split(".")
        self.assertEqual(len(parts), 3)
        header = json.loads(_b64url_decode(parts[0]))
        payload = json.loads(_b64url_decode(parts[1]))
        sig = _b64url_decode(parts[2])

        self.assertEqual(header, {"alg": "ES256", "kid": "ABCD123456", "typ": "JWT"})
        self.assertEqual(payload["iss"], "issuer-uuid")
        self.assertEqual(payload["aud"], "appstoreconnect-v1")
        self.assertGreaterEqual(payload["iat"], before)
        self.assertLessEqual(payload["iat"], after)
        self.assertEqual(payload["exp"] - payload["iat"], 20 * 60)

        # Signature is exactly 64 bytes: raw R || S, each 32 bytes.
        self.assertEqual(len(sig), 64)
        r = int.from_bytes(sig[:32], "big")
        s = int.from_bytes(sig[32:], "big")
        der = encode_dss_signature(r, s)
        signing_input = f"{parts[0]}.{parts[1]}".encode("ascii")
        public_key.verify(der, signing_input, ec.ECDSA(hashes.SHA256()))

    def test_rejects_rsa_key(self):
        from cryptography.hazmat.primitives.asymmetric import rsa
        rsa_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        pem = rsa_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        )
        with self.assertRaises(ValueError):
            mint_jwt("K", "I", pem)


class TestListBuilds(unittest.TestCase):
    def _build(self, bid: str, uploaded: str) -> dict:
        return {"id": bid, "attributes": {"uploadedDate": uploaded, "version": bid}}

    def test_since_filter(self):
        now = datetime.now(timezone.utc)
        page = {
            "data": [
                self._build("recent", (now - timedelta(hours=1)).isoformat()),
                self._build("old1", (now - timedelta(days=30)).isoformat()),
                self._build("old2", (now - timedelta(days=60)).isoformat()),
            ],
            "links": {"next": None},
        }
        with mock.patch.object(asc_client, "_request", return_value=page) as m:
            builds = list_builds("token", "app-1", since=now - timedelta(hours=24))
        self.assertEqual([b["id"] for b in builds], ["recent"])
        self.assertEqual(m.call_count, 1)

    def test_pagination(self):
        page1 = {
            "data": [{"id": "a", "attributes": {}}],
            "links": {"next": "https://api.appstoreconnect.apple.com/v1/builds?cursor=2"},
        }
        page2 = {
            "data": [{"id": "b", "attributes": {}}],
            "links": {"next": None},
        }
        with mock.patch.object(asc_client, "_request", side_effect=[page1, page2]) as m:
            builds = list_builds("token", "app-1")
        self.assertEqual([b["id"] for b in builds], ["a", "b"])
        self.assertEqual(m.call_count, 2)

    def test_rejects_off_host_pagination(self):
        page1 = {
            "data": [],
            "links": {"next": "https://evil.com/v1/builds?cursor=2"},
        }
        with mock.patch.object(asc_client, "_request", return_value=page1):
            with self.assertRaises(ASCError):
                list_builds("token", "app-1")

    def test_rejects_http_scheme_pagination(self):
        page1 = {
            "data": [],
            "links": {"next": "http://api.appstoreconnect.apple.com/v1/builds?cursor=2"},
        }
        with mock.patch.object(asc_client, "_request", return_value=page1):
            with self.assertRaises(ASCError):
                list_builds("token", "app-1")


class TestLoadCredentials(unittest.TestCase):
    def test_explicit_kwargs(self):
        creds = load_credentials(
            sops_path=None,
            key_id="K",
            issuer_id="I",
            app_id="A",
            key_pem=b"PEM-BYTES",
        )
        self.assertIsInstance(creds, Credentials)
        self.assertEqual(creds.key_id, "K")
        self.assertEqual(creds.issuer_id, "I")
        self.assertEqual(creds.app_id, "A")
        self.assertEqual(creds.key_pem, b"PEM-BYTES")

    def test_missing_kwargs_raises(self):
        with self.assertRaises(ValueError):
            load_credentials(sops_path=None, key_id="K")

    def test_sops_path(self):
        yaml_doc = (
            "key_id: KEYID123\n"
            "issuer_id: 11111111-2222-3333-4444-555555555555\n"
            "app_id: '1234567890'\n"
            "key_pem: |\n"
            "  -----BEGIN PRIVATE KEY-----\n"
            "  FAKE\n"
            "  -----END PRIVATE KEY-----\n"
        )
        fake_proc = subprocess.CompletedProcess(
            args=["sops", "-d", "/fake"],
            returncode=0,
            stdout=yaml_doc.encode("utf-8"),
            stderr=b"",
        )

        with mock.patch.object(Path, "is_file", return_value=True):
            creds = load_credentials(
                Path("/fake/path.yaml"),
                _runner=lambda *a, **kw: fake_proc,
            )

        self.assertEqual(creds.key_id, "KEYID123")
        self.assertEqual(creds.issuer_id, "11111111-2222-3333-4444-555555555555")
        self.assertEqual(creds.app_id, "1234567890")
        self.assertIn(b"BEGIN PRIVATE KEY", creds.key_pem)

    def test_sops_failure_raises(self):
        fake_proc = subprocess.CompletedProcess(
            args=["sops", "-d", "/fake"],
            returncode=1,
            stdout=b"",
            stderr=b"sops: not encrypted",
        )
        with mock.patch.object(Path, "is_file", return_value=True):
            with self.assertRaises(RuntimeError):
                load_credentials(
                    Path("/fake/path.yaml"),
                    _runner=lambda *a, **kw: fake_proc,
                )


if __name__ == "__main__":
    unittest.main()
