#!/usr/bin/env bash
# This file mirrors .github/workflows/ios.yml — any workflow edit to the
# test job check steps must update this script in the same PR, and vice
# versa. The xcodebuild invocations below are transcribed verbatim from that
# workflow, except for -destination: CI creates a simulator at job start and
# targets it by UDID, whereas locally we target the named simulator used
# throughout ios/AGENTS.md.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# check-test-dates.sh also scans ios/OwnPulseTests, though CI itself doesn't
# gate ios.yml on it (macOS runner minutes are precious) — run it locally so
# iOS test authors get the same signal backend/web CI enforces.
"$SCRIPT_DIR/check-test-dates.sh"

cd "$SCRIPT_DIR/../ios"

# Pin the toolchain so this script and CI build with the same Swift compiler.
# Override by exporting DEVELOPER_DIR before calling this script if you need
# a different local Xcode.
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode_16.4.app/Contents/Developer}"

XCPRETTY=(cat)
if command -v xcbeautify >/dev/null 2>&1; then
  XCPRETTY=(xcbeautify)
fi

set -o pipefail
xcodebuild test \
  -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16' \
  -clonedSourcePackagesDirPath .build \
  | "${XCPRETTY[@]}"

# The Debug test build above compiles single-file and does NOT run the whole-
# module region-isolation analysis. Swift 6 `sending` / actor-boundary errors
# therefore pass `test` but fail the Release archive — i.e. only surface at
# TestFlight time. Compile the app under Release (whole-module, like the
# archive) to catch that class of error before pushing.
xcodebuild build \
  -scheme OwnPulse \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -clonedSourcePackagesDirPath .build \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  CODE_SIGN_IDENTITY="" \
  | "${XCPRETTY[@]}"

echo "ci-ios.sh: all checks passed"
