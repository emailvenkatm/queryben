fn main() {
    tauri_build::build();

    // macOS dev signing runs OUTSIDE this file — two entry points cover the
    // two ways the dev binary can get launched:
    //
    //   1. `cargo run` / `cargo test` — intercepted by the cargo runner in
    //      src-tauri/.cargo/config.toml (scripts/dev-run.sh), which signs
    //      the binary then execs it.
    //   2. `pnpm tauri dev` — Tauri v2 calls `cargo build` (not `cargo run`)
    //      and then execs target/debug/queryben directly. That path skips
    //      the cargo runner. It's covered by the `build.runner` shim in
    //      tauri.conf.json (scripts/tauri-cargo-wrapper.sh), which signs
    //      after `cargo build` returns but BEFORE Tauri gets a chance to
    //      exec the binary.
    //
    // Both entry points sign synchronously — the old scheme in this file
    // was a detached poller that re-signed after launch, which cannot work:
    // the kernel captures code identity at exec() time, so a post-launch
    // re-sign leaves the running process under the linker's ad-hoc identity
    // (default keychain access group = per-binary-hash = re-prompt every
    // rebuild).
}
