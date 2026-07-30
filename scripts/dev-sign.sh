#!/usr/bin/env bash
#
# dev-sign.sh — sign the debug binary with a Developer ID identity so the
# macOS keychain access group is stable across rebuilds.
#
# The default (ad-hoc) signature keys the keychain access group on the
# binary hash, so every `pnpm tauri dev` rebuild invalidates any previously
# stored "Always Allow" grant AND makes older keychain items unreadable
# (the ACL check fails silently after the password prompt succeeds — user
# enters correct password, prompt closes, item still can't be read).
#
# Signing with a real Developer ID team lands keychain items in that team's
# stable per-team access group instead, so grants + items survive rebuilds.
#
# Invoked by tauri-cargo-wrapper.sh (post-`cargo build`) and by
# dev-run.sh (pre-exec via `.cargo/config.toml` runner). Idempotent —
# safe to run multiple times on the same binary.

set -euo pipefail

BIN="${1:-}"

if [ -z "$BIN" ]; then
  echo "dev-sign: usage: $0 <binary-path>" >&2
  exit 2
fi

if [[ "$(uname)" != "Darwin" ]]; then
  exit 0
fi

if [ ! -f "$BIN" ]; then
  echo "dev-sign: binary not found at $BIN" >&2
  exit 2
fi

# Prefer Venkata's personal Developer ID (QueryBen team). Fall back to any
# Developer ID Application identity on the machine, so contributors with
# their own certs can still get stable dev builds.
PREFERRED="Developer ID Application: Venkata Maguluri (867PDSG8CJ)"
IDENTITY=""

if security find-identity -v -p codesigning 2>/dev/null | grep -q "$PREFERRED"; then
  IDENTITY="$PREFERRED"
else
  IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null \
    | grep 'Developer ID Application' \
    | head -1 \
    | sed -n 's/.*"\(.*\)"/\1/p' || true)"
fi

if [ -z "$IDENTITY" ]; then
  echo "dev-sign: no Developer ID Application identity found in keychain" >&2
  echo "  install one via developer.apple.com, then re-run" >&2
  exit 1
fi

# --force to overwrite the ad-hoc sig cargo/linker left. Skip --timestamp
# for dev builds (no notarization here; timestamp server adds latency and
# is only required for release/notarize).
codesign --force --sign "$IDENTITY" --timestamp=none "$BIN" >/dev/null 2>&1
