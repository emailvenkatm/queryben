# QueryBen

A SQL client for macOS, Windows, and Linux. Built for the Azure SQL crowd
after Microsoft killed Azure Data Studio.

Fast enough to feel local. Signed into your Azure account so the AAD dance
is one click. Notebooks, saved queries, schema compare, and an AI panel
if you enable it.

Works with Azure SQL and SQL Server today. Postgres, MySQL, and SQLite
are on deck.

## Running from source

Prereqs: node 22+, pnpm, rust stable.

    pnpm install
    pnpm tauri dev

## Building a release

    pnpm tauri build

macOS notarization needs an Apple Developer ID cert. See `scripts/`.

## Contributing

Issues welcome. PRs even more so. Read `docs/ARCHITECTURE.md` before
touching structure.
