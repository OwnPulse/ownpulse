#!/usr/bin/env bash
# This file mirrors .github/workflows/ios.yml — any workflow edit to the
# test job check steps must update this script in the same PR, and vice
# versa. The xcodebuild invocations below are transcribed verbatim from that
# workflow, except: -destination targets the named simulator used throughout
# ios/AGENTS.md (CI creates one at job start and targets it by UDID), and
# output formatting falls back to cat when xcbeautify isn't installed.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# check-test-dates.sh also scans ios/OwnPulseTests, though CI itself doesn't
# gate ios.yml on it (macOS runner minutes are precious) — run it locally so
# iOS test authors get the same signal backend/web CI enforces.
"$SCRIPT_DIR/check-test-dates.sh"

cd "$SCRIPT_DIR/../ios"

# Pin the toolchain so this script and CI build with the same Swift compiler.
# Override by exporting DEVELOPER_DIR before calling this script if you need
# a different local Xcode. Local machines install Xcode at /Applications/
# Xcode.app rather than the runner image's versioned path, so fall back to
# the active xcode-select toolchain when the CI path is absent.
CI_XCODE=/Applications/Xcode_26.6.app/Contents/Developer
if [ -z "${DEVELOPER_DIR:-}" ]; then
  if [ -d "$CI_XCODE" ]; then
    DEVELOPER_DIR="$CI_XCODE"
  else
    DEVELOPER_DIR="$(xcode-select -p)"
    echo "ci-ios.sh: $CI_XCODE not found; using $DEVELOPER_DIR" >&2
  fi
fi
export DEVELOPER_DIR

# Everything behind `#if swift(>=6.3)` compiles to nothing on older
# toolchains, so a green run there would not cover that code. Fail rather
# than pass vacuously.
XCODE_MAJOR=$(xcodebuild -version | awk 'NR==1 {print int($2)}')
if [ "$XCODE_MAJOR" -lt 26 ]; then
  echo "ci-ios.sh: error: selected toolchain is Xcode $XCODE_MAJOR; CI builds with Xcode 26+ (Swift 6.3)." >&2
  echo "ci-ios.sh: export DEVELOPER_DIR pointing at an Xcode 26+ install and re-run." >&2
  exit 1
fi

# The named destination below only resolves if an "iPhone 16" simulator
# exists; fresh Xcode installs create only current-generation devices. The
# simulator must also run iOS 26+, because the medication tests guard on
# `#available(iOS 26.0, *)` and pass vacuously on older runtimes.
SIM_IOS_MAJOR=$(xcrun simctl list devices available \
  | awk '/^-- iOS/ {ver=$3} /iPhone 16 \(/ {print int(ver)}' | sort -n | tail -1)
if [ -z "$SIM_IOS_MAJOR" ]; then
  echo "ci-ios.sh: error: no 'iPhone 16' simulator found. Create one with:" >&2
  echo "  xcrun simctl create 'iPhone 16' com.apple.CoreSimulator.SimDeviceType.iPhone-16" >&2
  exit 1
fi
if [ "$SIM_IOS_MAJOR" -lt 26 ]; then
  echo "ci-ios.sh: error: newest 'iPhone 16' simulator runs iOS $SIM_IOS_MAJOR; iOS 26+ required." >&2
  echo "ci-ios.sh: create one on a current runtime with 'xcrun simctl create'." >&2
  exit 1
fi

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
