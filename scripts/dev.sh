#!/usr/bin/env bash
#
# dev.sh — kill any stale queryben dev process, then start tauri dev.
# Stale processes hold onto port 1420 (vite) and the sqlite queries.db lock;
# skipping this leaves you troubleshooting a phantom "already listening" error.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV_BIN="$REPO_ROOT/src-tauri/target/debug/queryben"

if [ -x "$DEV_BIN" ]; then
  pkill -f "$DEV_BIN" 2>/dev/null || true
fi

cd "$REPO_ROOT"
exec pnpm tauri dev
