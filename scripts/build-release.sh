#!/usr/bin/env bash
#
# build-release.sh — local release build. Runs `pnpm tauri build` with the
# current-arch target so you get a signable .app / .msi / .AppImage under
# src-tauri/target/release/bundle/.
#
# macOS: reads the first Developer ID Application cert on the machine
# (same identity resolution as dev-sign.sh) and passes it via the env vars
# Tauri's bundler recognizes. Notarization is NOT run here — that's a CI
# concern, and locally you usually just want a signed .app to smoke-test.
#
# Windows / Linux: no signing done locally. Ship via CI.
#
# Usage:
#   scripts/build-release.sh                # host arch
#   scripts/build-release.sh --target aarch64-apple-darwin
#   scripts/build-release.sh --target x86_64-apple-darwin

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

if [[ "$(uname)" == "Darwin" ]] && [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  # Prefer Venkata's personal Developer ID (QueryBen team 867PDSG8CJ). Fall
  # back to any Developer ID Application identity so contributors with their
  # own certs still get a signed build. Mirrors scripts/dev-sign.sh — without
  # this preference, `security find-identity` sorts Injoya LLC first and
  # QueryBen ends up cross-signed under the wrong team.
  PREFERRED="Developer ID Application: Venkata Maguluri (867PDSG8CJ)"
  IDENTITY_NAME=""

  if security find-identity -v -p codesigning 2>/dev/null | grep -q "$PREFERRED"; then
    IDENTITY_NAME="$PREFERRED"
  else
    IDENTITY_NAME="$(security find-identity -v -p codesigning 2>/dev/null \
      | grep 'Developer ID Application' \
      | head -1 \
      | sed -n 's/.*"\(.*\)"/\1/p' || true)"
  fi

  if [ -n "$IDENTITY_NAME" ]; then
    export APPLE_SIGNING_IDENTITY="$IDENTITY_NAME"
    echo "build-release: using $APPLE_SIGNING_IDENTITY"
  else
    echo "build-release: no Developer ID Application cert found, build will be unsigned" >&2
  fi
fi

exec pnpm tauri build "$@"
