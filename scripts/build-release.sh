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
  # Same resolution as dev-sign.sh: first Developer ID Application cert.
  # Tauri's bundler picks this up when calling codesign.
  IDENTITY_LINE="$(security find-identity -v -p codesigning 2>/dev/null \
    | grep 'Developer ID Application' \
    | head -1 || true)"

  if [ -n "$IDENTITY_LINE" ]; then
    # Common Name is the quoted field on the line.
    IDENTITY_NAME="$(printf '%s\n' "$IDENTITY_LINE" | sed -n 's/.*"\(.*\)"/\1/p')"
    export APPLE_SIGNING_IDENTITY="$IDENTITY_NAME"
    echo "build-release: using $APPLE_SIGNING_IDENTITY"
  else
    echo "build-release: no Developer ID Application cert found — build will be unsigned" >&2
  fi
fi

exec pnpm tauri build "$@"
