#!/usr/bin/env bash
#
# dev-run.sh — cargo runner for macOS dev builds.
#
# Signs the binary with Developer ID before exec so the kernel captures the
# correct code identity. macOS keychain ACL checks happen at exec time, not
# at disk-write time, so signing after launch is too late.
#
# Invoked by cargo via .cargo/config.toml runner setting. Receives the
# binary path + any args cargo would pass to it.

set -euo pipefail

BIN="$1"
shift

if [[ "$(uname)" != "Darwin" ]]; then
  exec "$BIN" "$@"
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIGN_SCRIPT="$REPO_ROOT/scripts/dev-sign.sh"

if [ -x "$SIGN_SCRIPT" ]; then
  "$SIGN_SCRIPT" "$BIN" 2>/dev/null || true
fi

exec "$BIN" "$@"
