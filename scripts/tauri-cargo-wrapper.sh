#!/usr/bin/env bash
#
# tauri-cargo-wrapper.sh — cargo shim wired via tauri.conf.json `build.runner`
# so we can sign the debug binary AFTER cargo build but BEFORE Tauri execs it.
#
# WHY THIS EXISTS (read before "simplifying"):
#
# `pnpm tauri dev` invokes `cargo build` (not `cargo run`), then Tauri's own
# harness execs `src-tauri/target/debug/queryben` directly. That path skips
# our `.cargo/config.toml` runner (which only fires for `cargo run` /
# `cargo test`), so the dev binary launches under the linker's ad-hoc
# signature. Ad-hoc = fresh binary hash every build = fresh default keychain
# access group = "queryben wants to use com.queryben.azure" prompt on every
# rebuild, forever, no matter how many times you click "Always Allow".
#
# Tauri v2 exposes `build.runner` in tauri.conf.json — it lets us replace the
# `cargo` binary Tauri invokes. We forward every arg straight to real cargo,
# and on a successful debug `cargo build ...` we synchronously sign the
# emitted binary with our Developer ID identity. That signing happens BEFORE
# this script returns, which is BEFORE Tauri execs the binary. Ordering is
# deterministic — no polling, no race window.
#
# Rejected alternatives (see docs/DEVELOPMENT.md + prior commits):
#   * build.rs post-emit sign — build.rs runs BEFORE link, not after. No
#     stable macOS post-link hook exists (Linux's -Wl,--post-link is
#     ld-only). Would require a detached poller = racy = already-tried-
#     and-reverted.
#   * `beforeDevCommand` chain — runs BEFORE cargo, wrong side of the order.
#   * Background watcher on target/debug/queryben — racy same as the poller.
#   * `keychain-access-groups` entitlement — SIGKILL exit 137 on unnotarized
#     dev binaries (hardened runtime + provisioning-profile requirement).
#
# Usage: invoked by Tauri CLI, not by humans. Args are whatever cargo
# subcommand + flags Tauri decides to run (typically `build --no-default-
# features --color always --manifest-path .../Cargo.toml [--release]`).
#
# Escape hatch: `QUERYBEN_SKIP_DEV_SIGN=1` forwards to cargo without signing.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIGN_SCRIPT="$REPO_ROOT/scripts/dev-sign.sh"

# Locate real cargo. `command -v` respects the PATH Tauri gives us, which is
# what the developer would see if they typed `cargo` themselves.
REAL_CARGO="$(command -v cargo)"
if [ -z "$REAL_CARGO" ]; then
  echo "tauri-cargo-wrapper: no 'cargo' on PATH" >&2
  exit 127
fi

# Forward EVERY arg to real cargo. Capture the subcommand so we know whether
# to sign after (only debug `build`s produce the dev binary we care about).
SUBCOMMAND="${1:-}"
IS_RELEASE=0
for arg in "$@"; do
  if [ "$arg" = "--release" ]; then
    IS_RELEASE=1
    break
  fi
done

"$REAL_CARGO" "$@"
CARGO_EXIT=$?

# Only post-process on successful DEBUG builds. `tauri build` passes
# `--release` and runs its own notarization-ready sign step in the bundler;
# re-signing here would just add noise (and break release sig if we picked a
# different identity).
if [ "$CARGO_EXIT" -ne 0 ] \
  || [ "$SUBCOMMAND" != "build" ] \
  || [ "$IS_RELEASE" = "1" ] \
  || [ "${QUERYBEN_SKIP_DEV_SIGN:-}" = "1" ] \
  || [[ "$(uname)" != "Darwin" ]]; then
  exit "$CARGO_EXIT"
fi

# Extract --target-dir if Tauri passed one (it usually doesn't for dev, but
# be defensive). Otherwise default to src-tauri/target — matches the layout
# Tauri assumes when it looks for the emitted binary.
TARGET_DIR=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--target-dir" ]; then
    TARGET_DIR="$arg"
    break
  fi
  prev="$arg"
done
if [ -z "$TARGET_DIR" ]; then
  TARGET_DIR="$REPO_ROOT/src-tauri/target"
fi

DEV_BIN="$TARGET_DIR/debug/queryben"
if [ ! -x "$DEV_BIN" ]; then
  # Cargo may have built only the lib (e.g. `cargo build -p queryben_lib`).
  # Nothing to sign — succeed silently so we don't fail unrelated builds.
  exit 0
fi

if [ ! -x "$SIGN_SCRIPT" ]; then
  echo "tauri-cargo-wrapper: signer not found at $SIGN_SCRIPT — dev binary" >&2
  echo "  will launch ad-hoc-signed and keychain WILL re-prompt on rebuild" >&2
  exit 0
fi

# Synchronous sign — returns BEFORE Tauri execs the binary, so the kernel
# captures the Developer-ID identity at launch time (not the linker's
# ad-hoc sig). dev-sign.sh is idempotent + macOS-guarded internally.
if ! "$SIGN_SCRIPT" "$DEV_BIN"; then
  echo "tauri-cargo-wrapper: dev-sign.sh failed — refusing to hand off an" >&2
  echo "  ad-hoc-signed binary to Tauri (keychain would re-prompt)" >&2
  exit 70
fi

exit 0
