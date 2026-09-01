#!/usr/bin/env bash
# This file mirrors .github/workflows/web.yml — any workflow edit to the test
# job check steps must update this script in the same PR, and vice versa.
# Commands below are transcribed verbatim from that workflow.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# --- test-date-lint job ---
"$REPO_ROOT/scripts/check-test-dates.sh"

cd "$REPO_ROOT/web"

npm ci

# The design-tokens generator test imports style-dictionary, which lives in
# tools/design-tokens/node_modules (resolved relative to build.js), not
# web/node_modules. Install it so the generator + idempotency tests run.
(cd "$REPO_ROOT/tools/design-tokens" && npm ci)

# Gate the palette on WCAG 2.1 AA.
(cd "$REPO_ROOT/tools/design-tokens" && npm run check:contrast)

npx tsc --noEmit

npx biome check src/ tests/

npx vitest run --coverage

if compgen -G "tests/e2e/*.spec.*" > /dev/null; then
  npx playwright install --with-deps chromium
  npx playwright test
else
  echo "No E2E test files in tests/e2e/, skipping"
fi

echo "ci-web.sh: all checks passed"
