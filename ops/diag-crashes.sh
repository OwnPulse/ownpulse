#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) OwnPulse Contributors
#
# diag-crashes.sh — thin shim. All logic lives in asc_client.py diagnose.
set -euo pipefail
exec python3 "$(dirname "$0")/asc_client.py" diagnose "$@"
