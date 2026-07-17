#!/usr/bin/env bash
#
# verify.sh — the same checks CI runs, on your laptop, before you push.
# Faster feedback than waiting on Actions.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> pnpm tsc --noEmit"
pnpm tsc --noEmit

echo "==> cargo check"
cargo check --manifest-path src-tauri/Cargo.toml

echo "==> cargo test --lib"
cargo test --manifest-path src-tauri/Cargo.toml --lib

# Integration tests. Skip the keychain-real ones; they SIGKILL under
# cargo's per-hash test binary signature (see MEMORY: tests must never
# touch the real macOS keychain).
echo "==> cargo test --tests (skip needs_real_keychain)"
cargo test --manifest-path src-tauri/Cargo.toml --tests -- --skip needs_real_keychain

echo "ok"
