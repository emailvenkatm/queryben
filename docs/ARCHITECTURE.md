# Architecture

QueryBen is a Tauri 2 desktop SQL client. Rust does IO, React draws pixels, everything else is glue.

The old codebase grew fast and it shows — this rewrite is about tightening the seams, not moving them.

## Layout

### Backend (`src-tauri/src/`)

```
main.rs             entry
lib.rs              tauri builder + command registration
error.rs            AppError enum, single Result alias
state.rs            AppState (registry, providers, background runners)

core/               pure types, no IO, no tokio
  connection.rs
  query.rs
  notebook.rs
  ...

adapters/           IO. keychain, sqlite, mssql, azure oauth, filesystem.
  mssql.rs
  keychain/
    macos.rs
    linux.rs
    windows.rs
  azure/
    oauth.rs        interactive + refresh
    rest.rs         management api calls
    accounts.rs     account cache on disk
  ...

app/                use-cases. thin glue that pulls adapters together.
  sign_in.rs
  execute_query.rs
  import_from_ads.rs
  ...

ipc/                #[tauri::command] wrappers. dumb. only marshal + delegate.
  connection.rs
  query.rs
  ...
```

Why the split: `#[tauri::command]` functions stay boring, and the interesting logic (retry, cache, error mapping) lives in `app/` where it's callable from tests without Tauri.

### Frontend (`src/`)

```
main.tsx
App.tsx             providers + router

shared/
  ui/               design-system primitives (Button, Input, Dialog, Tabs...)
  hooks/            useHotkey, useLocalStorage, ...
  lib/              pure utilities. no react.
  api/              tauri IPC client + generated bindings
  types.ts          cross-feature types (rare — most types belong to features)

features/
  connections/
    components/
    hooks/
    api.ts          feature-local mutations/queries
    types.ts
    index.ts        the public API. no deep imports elsewhere.
  query-editor/
  notebook/
  ...

widgets/            multi-feature layout pieces (AppShell, ObjectExplorer)
pages/              route entry points
```

Rules:
- Features import from `shared/` and their own subtree. **Never** from another feature's internals — only through that feature's `index.ts`.
- No barrel exports from `shared/*` — import the file you need.
- Colocate. If `PaymentForm.tsx` needs a helper only it uses, put it next to it.

## Naming

- Files: `kebab-case.ts` for utilities, `PascalCase.tsx` for components, `use-camel-case.ts` for hooks.
- Rust: `snake_case.rs`. Types are `PascalCase`. Functions are verb-first: `load_connection`, not `connection_loader` or `get_connection_data`.
- No `Manager`, `Service`, `Handler`, `Helper` suffixes unless they mean something specific in the domain.

## Errors

- **Rust**: one `AppError` enum in `error.rs`. Variants are semantic (`ConnectionRefused`, `AuthExpired`, `TableNotFound`), not stringly-typed. `type Result<T> = std::result::Result<T, AppError>;`
- **TS**: errors from IPC are typed via specta bindings. Never `catch (e: any)`. Use `formatAppErrorForDisplay(e)` from `shared/api/errors.ts` when rendering.

## IDs

Real newtypes, not `String`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub Uuid);
```

Applies to `AccountId`, `NotebookId`, `QueryId`. UI can treat them as strings; Rust cannot lose them.

## State

- Server state → TanStack Query. That includes anything from Rust.
- Cross-feature client state → Zustand. Small stores, one per concern (`active-connection`, `theme`).
- Feature-local state → `useState`. If two components in a feature need it, `useContext` inside that feature. If it grows, it's a Zustand store.
- Nothing goes in `localStorage` except explicit persistence (theme, onboarding-seen, layout). Everything else re-derives.

## Testing

- Rust: unit tests colocated (`#[cfg(test)] mod tests` at bottom of file). Integration tests in `src-tauri/tests/` run `app/` use-cases against fixtures.
- TS: Vitest. Test the hooks, not the components. Test the API layer against a mocked Tauri IPC.
- **Real-DB smoke**: `scripts/smoke-real.sh` runs a subset of integration tests against `dbclient-testdb.database.windows.net` — must pass before shipping.

## Commits

- Multiple small commits per feature, not one giant one.
- Real dev rhythm: `wip:`, `fix:`, `refactor:`, `chore:` prefixes. Occasional revert or amend is fine and looks real.
- Commit messages under 72 chars. Body optional and only when the why isn't obvious.
- No `` lines. This is our code.
