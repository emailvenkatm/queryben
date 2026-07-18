# workflows

`ci.yml` runs on every PR + push to main. Type-check, `cargo check`,
`cargo test --lib`, and integration tests (skipping the ones that need
a real keychain). Full matrix: mac arm64, mac x86_64, windows, linux.

`release.yml` runs on tag push (`v*`). Builds signed installers for all
four targets via `tauri-apps/tauri-action` and uploads them as a draft
GitHub Release. Bump `package.json` + `Cargo.toml`, tag, push:

    git tag v0.1.0
    git push origin v0.1.0

## secrets

macOS signing + notarization:

- `APPLE_CERT_P12_BASE64` — base64 of the Developer ID Application .p12
- `APPLE_CERT_PASSWORD` — password used when exporting the .p12
- `APPLE_SIGNING_IDENTITY` — the cert Common Name (e.g. `Developer ID Application: Foo (TEAMID)`)
- `APPLE_ID` — Apple ID email
- `APPLE_ID_PASSWORD` — app-specific password from appleid.apple.com
- `APPLE_TEAM_ID` — 10-char team ID

Windows signing (optional; build is unsigned if absent):

- `WINDOWS_CERT_P12_BASE64`
- `WINDOWS_CERT_PASSWORD`

Linux is unsigned.
