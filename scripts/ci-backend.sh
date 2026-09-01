#!/usr/bin/env bash
# This file mirrors .github/workflows/backend.yml — any workflow edit to the
# fmt/clippy/test job check steps must update this script in the same PR,
# and vice versa. Commands below are transcribed verbatim from that workflow.
#
# Requires: a reachable Postgres (DATABASE_URL set), sqlx-cli, cargo-llvm-cov.
# See CLAUDE.md "Local Setup" for spinning up Postgres via Docker.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- test-date-lint job ---
"$SCRIPT_DIR/check-test-dates.sh"

cd "$SCRIPT_DIR/../backend"

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is not set. Example:" >&2
  echo '  export DATABASE_URL=postgres://ownpulse:ownpulse@localhost:5432/ownpulse' >&2
  exit 1
fi

# --- fmt job ---
cargo fmt --check

# --- clippy job ---
cargo clippy -- -D warnings

# --- test job ---
sqlx migrate run --source ../db/migrations

cargo sqlx prepare --workspace --check

# Pact provider-verification gate: fails if the backend drifts from a
# committed consumer contract.
cargo test --test contract

cargo llvm-cov --all-targets --lcov --output-path lcov.info
cargo llvm-cov report --summary-only

echo "ci-backend.sh: all checks passed"
