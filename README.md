# QueryBen

A native SQL client for SQL Server and Azure SQL, for people who lived in
Azure Data Studio and don't want to switch to VS Code + a plugin soup now
that Microsoft has retired it (Feb 2026).

Tauri 2 shell, tiberius on the Rust side, React 19 on the front.

<!-- Add a screenshot here once the UI settles. Drop a PNG in docs/ and
     replace this comment with: ![QueryBen](docs/screenshot.png) -->

## Install

First release is going out shortly. Once it lands, grab a build from
[Releases](https://github.com/emailvenkatm/queryben/releases):

- macOS: `.dmg` (Apple Silicon and Intel), signed and notarized
- Windows: `.msi`, EV-signed
- Linux: `.AppImage` and `.deb`, tested on Ubuntu 22.04+ and Fedora 40

## Run from source

Node 22+, pnpm, Rust stable.

```
pnpm install
cp .env.example .env   # fill in Azure AD client + tenant IDs
pnpm tauri dev
```

Build a release with `pnpm tauri build`. macOS notarization needs a
Developer ID cert — see `scripts/`.

## What works today

- Azure AD sign-in via loopback + PKCE, refresh tokens in the OS keychain
- SQL Server and Azure SQL over tiberius
- Object explorer (databases, schemas, tables, views, procs)
- Query editor with Monaco, results in a virtualized grid
- Notebooks with markdown + SQL cells
- Schema compare between two connections
- Firewall auto-heal for Azure error 40615

## What's next

- PostgreSQL, MySQL, SQLite
- Query cancellation via tiberius attention tokens
- Optional AI panel for query explain and rewrite

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Architecture notes in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## License

MIT. See [LICENSE](LICENSE).
