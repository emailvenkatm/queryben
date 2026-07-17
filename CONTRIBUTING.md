# Contributing

Thanks for looking. QueryBen is small enough that a good PR usually lands
same-week.

## Run it locally

```
pnpm install
cp .env.example .env   # add your Azure AD client + tenant IDs
pnpm tauri dev
```

Node 22+, pnpm, Rust stable. The `.env` values are only needed if you want
to exercise Azure sign-in; the app still boots without them.

## How the code is organized

- `src/` — React frontend. Feature slices under `src/features/`, shared
  primitives under `src/shared/`. Never import another feature's internals
  — go through its `index.ts`.
- `src-tauri/src/` — Rust backend. `core/` is pure types, `adapters/` is
  IO, `app/` is orchestration, `ipc/` is the thin `#[tauri::command]`
  surface. Full breakdown in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Sending a PR

Small and focused wins. One conceptual change per PR is easier to review
than a 40-file drop.

- Rebase onto `main` before opening.
- `pnpm lint` and `pnpm type-check` need to pass. `cargo test` too, if you
  touched Rust.
- Commit style is `area: verb noun`, lowercase, no period:
  `query: cancel via attention token`, not `Add query cancellation`.
- Keep the message body for the why, not the what — the diff is the what.
- No `` or similar attribution lines.

If the change is bigger than a couple hundred lines, open an issue first
so we can agree on shape before you sink time into it.

## Reporting bugs

Include:

- OS and version
- Connection type (SQL Server on-prem, Azure SQL, etc.) and auth mode
- What you did, what happened, what you expected
- A minimal repro if you can — even a screenshot of the error toast helps

Auth issues: mention whether it's a work/school tenant, personal MSA, or
guest, and whether `.env` has an explicit tenant GUID or `/organizations`.
That combination is usually the culprit.

## Code style

Names do the work; comments explain *why*, not *what*. `rustfmt` and
`prettier` on save. `?` beats `match err { ... }` in Rust when you're just
propagating. `unknown` beats `any` in TS — refine at the boundary. Small
components, small functions; if a file is over ~150 lines it usually wants
a split.
