//! Connection model + disk-backed registry.
//!
//! Persistence layout:
//!   - `{app_data_dir}/connections.json` — one JSON array of PersistedEntry,
//!     no secrets. Written atomically (tmp file + rename).
//!   - OS keychain (service = `queryben.connection`, account = connection UUID) —
//!     the SQL password, if any. AAD entries never touch the keychain.
//!
//! On startup we load the JSON, then best-effort read passwords from the
//! keychain. Missing keychain entries do not drop the connection — they just
//! leave `password: None` and the user gets prompted at reopen.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

// Service name for the OS keychain (macOS Keychain / Windows Credential
// Manager / Linux Secret Service). Distinct from the OAuth service so
// `security find-generic-password -s queryben.connection` only lists
// connection passwords.
const KEYRING_SERVICE: &str = "queryben.connection";

// File name inside the Tauri app data directory.
const REGISTRY_FILE: &str = "connections.json";
const REGISTRY_TMP: &str = "connections.json.tmp";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    SqlAuth,
    // Bearer scoped for database.windows.net/.default. Not persisted; expires
    // in ~1h and gets re-minted from the refresh token.
    AadToken,
    AadPassword,
    AadInteractive,
    AadManagedIdentity,
}

impl AuthMode {
    // True for auth modes that authenticate to Azure SQL by handing tiberius a
    // bearer minted via the MSAL browser-popup / PKCE flow. `AadToken` and
    // `AadInteractive` share this path — the label distinction is UI-only
    // (users recognise "AAD Interactive" from SSMS/ADS) but the runtime code
    // path is identical: acquire a token via `azure_oauth::acquire_token` and
    // set `AuthMethod::aad_token`.
    pub fn uses_aad_bearer(&self) -> bool {
        matches!(self, AuthMode::AadToken | AuthMode::AadInteractive)
    }
}

// User-picked color tag. Serialized as lowercase string so on-disk JSON stays
// human-readable and the frontend can match on it directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionColor {
    Cream,
    Amber,
    Jade,
    Rose,
    Violet,
    Graphite,
}

pub const NICKNAME_MAX_LEN: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
    pub server: String,
    pub database: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub auth_mode: AuthMode,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    // MSAL `home_account_id` of the Azure account that minted this bearer.
    // Nullable so legacy AAD connections (created before multi-account) still
    // deserialize; the migration path in `state::AppState::new` backfills the
    // field to whatever's in the account registry when there's exactly one
    // candidate.
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub color: Option<ConnectionColor>,
}

impl Connection {
    pub fn display_name(&self) -> &str {
        self.nickname
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.server)
    }
}

// Normalize a caller-provided nickname: trim, treat empty as None, enforce
// the length cap. Returns Validation on overflow so the frontend can surface
// a field-level message instead of a raw Internal.
pub fn normalize_nickname(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(s) = raw else { return Ok(None) };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > NICKNAME_MAX_LEN {
        return Err(AppError::Validation(format!(
            "nickname must be {NICKNAME_MAX_LEN} characters or fewer"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionInput {
    pub name: String,
    pub server: String,
    pub database: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub auth_mode: AuthMode,
    #[serde(default)]
    pub trust_server_certificate: bool,
    // skip_serializing so an accidental log or response never leaks the bearer.
    #[serde(default, skip_serializing)]
    pub aad_bearer: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub color: Option<ConnectionColor>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionInput {
    pub id: Uuid,
    // `Option<Option<T>>` semantics via a plain `Option<String>`: `None` means
    // "leave as-is" (field omitted from the JSON payload), `Some("")` clears.
    // Keeping the wire shape flat because the frontend only edits nickname +
    // color today.
    #[serde(default)]
    pub nickname: Option<String>,
    // Sentinel-free clear: pass `null` in JSON and the field lands as
    // `Some(None)`. serde's flatten of `Option<Option<T>>` is fine on stable
    // via the `deserialize_with` pattern, but we keep it simple: an explicit
    // "clearColor: true" flag documents intent.
    #[serde(default)]
    pub color: Option<ConnectionColor>,
    #[serde(default)]
    pub clear_nickname: bool,
    #[serde(default)]
    pub clear_color: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: Option<String>,
    pub latency_ms: Option<u32>,
}

// The active tiberius Client isn't cached here because its generic parameter
// makes the AppState boundary painful. We reopen per-query for now; a real
// pool lands later.
pub struct ConnectionEntry {
    pub connection: Connection,
    pub password: Option<String>,
    pub trust_server_certificate: bool,
    // Present for AadToken entries so reopen can re-mint the sqldb bearer
    // silently via the keychain-backed refresh token.
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    // ARM resource ID of the underlying Azure SQL server, e.g.
    // "/subscriptions/xxx/resourceGroups/rg/providers/Microsoft.Sql/servers/foo".
    // Set from the connect wizard for AAD connections, and cached lazily by
    // the auto-firewall-fix path so we don't re-scan every subscription on
    // every 40615. `None` is the honest default for SQL-auth entries and for
    // AAD connections created before this field existed.
    pub server_arm_id: Option<String>,
}

// On-disk shape. `password` intentionally absent — it lives in the OS keychain.
// serde(rename_all = "camelCase") so the file matches the same convention as
// the IPC-facing `Connection` struct.
//
// `Connection` already carries `account_id` (via `#[serde(flatten)]`), so we
// don't repeat it here — the flatten pulls the field into the JSON row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedEntry {
    #[serde(flatten)]
    connection: Connection,
    #[serde(default)]
    trust_server_certificate: bool,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    client_id: Option<String>,
    // Persisted so the ARM discovery cost is amortized across app restarts.
    // Never fabricated — we only ever write what came back from the ARM API,
    // which means the auto-firewall path never sends garbage to Azure.
    #[serde(default)]
    server_arm_id: Option<String>,
}

impl PersistedEntry {
    fn from_entry(entry: &ConnectionEntry) -> Self {
        Self {
            connection: entry.connection.clone(),
            trust_server_certificate: entry.trust_server_certificate,
            tenant_id: entry.tenant_id.clone(),
            client_id: entry.client_id.clone(),
            server_arm_id: entry.server_arm_id.clone(),
        }
    }
}

pub struct ConnectionRegistry {
    inner: Mutex<HashMap<Uuid, ConnectionEntry>>,
    // Fully-qualified path to connections.json. Set once at construction so
    // callers don't have to plumb the AppHandle through every mutation.
    file_path: PathBuf,
}

impl ConnectionRegistry {
    /// Build a registry rooted at `app_data_dir`. Creates the directory if it
    /// doesn't exist. Loads any pre-existing `connections.json` and re-hydrates
    /// passwords from the OS keychain best-effort.
    pub fn new(app_data_dir: &Path) -> Result<Self, AppError> {
        // Missing app_data_dir on first launch is expected. `create_dir_all`
        // is a no-op if it already exists.
        fs::create_dir_all(app_data_dir).map_err(|e| {
            AppError::internal(format!(
                "create app data dir {}: {e}",
                app_data_dir.display()
            ))
        })?;

        let file_path = app_data_dir.join(REGISTRY_FILE);
        let inner = load_from_disk(&file_path)?;

        Ok(Self {
            inner: Mutex::new(inner),
            file_path,
        })
    }

    pub fn insert(&self, entry: ConnectionEntry) -> Result<Connection, AppError> {
        let conn = entry.connection.clone();
        let id = conn.id;
        let password = entry.password.clone();

        {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| AppError::internal("registry mutex poisoned"))?;
            guard.insert(id, entry);
            // Write JSON while we still hold the lock so a concurrent mutation
            // can't race a stale snapshot to disk.
            persist_locked(&self.file_path, &guard)?;
        }

        // Keychain write happens after the JSON commit so an interrupted
        // insert leaves either "nothing" or "JSON entry, prompt for password"
        // — never "orphan password with no JSON row".
        if let Some(pw) = password {
            // Best-effort: a failed keychain write shouldn't kill the insert
            // (user can still use the connection this session; on next launch
            // they'll be prompted for the password).
            if let Err(err) = kc_store(&id.to_string(), &pw) {
                tracing::warn!(
                    target: "queryben::registry::insert",
                    %id,
                    "keychain write failed: {err}"
                );
            }
        }

        Ok(conn)
    }

    pub fn list(&self) -> Result<Vec<Connection>, AppError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        Ok(guard.values().map(|e| e.connection.clone()).collect())
    }

    pub fn remove(&self, id: Uuid) -> Result<(), AppError> {
        {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| AppError::internal("registry mutex poisoned"))?;
            guard
                .remove(&id)
                .ok_or_else(|| AppError::NotFound(format!("connection {id}")))?;
            persist_locked(&self.file_path, &guard)?;
        }

        // Keychain cleanup is best-effort — an orphaned keychain entry is
        // harmless (a future insert reusing this UUID would overwrite it, and
        // v4 collisions are practically zero).
        if let Err(err) = kc_delete(&id.to_string()) {
            tracing::warn!(
                target: "queryben::registry::remove",
                %id,
                "keychain delete failed: {err}"
            );
        }

        Ok(())
    }

    /// Clone the entry so callers can open a fresh Client without holding
    /// the registry lock through network IO.
    pub fn snapshot(&self, id: Uuid) -> Result<ConnectionSnapshot, AppError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        let entry = guard
            .get(&id)
            .ok_or_else(|| AppError::NotFound(format!("connection {id}")))?;
        Ok(ConnectionSnapshot {
            connection: entry.connection.clone(),
            password: entry.password.clone(),
            trust_server_certificate: entry.trust_server_certificate,
            tenant_id: entry.tenant_id.clone(),
            client_id: entry.client_id.clone(),
            server_arm_id: entry.server_arm_id.clone(),
        })
    }

    /// Apply an in-place patch to the connection's user-facing labels.
    /// Returns the updated `Connection` so the frontend can round-trip the
    /// new fields without a second `list` call.
    pub fn update_labels(
        &self,
        input: UpdateConnectionInput,
    ) -> Result<Connection, AppError> {
        let nickname = normalize_nickname(input.nickname)?;
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        let entry = guard
            .get_mut(&input.id)
            .ok_or_else(|| AppError::NotFound(format!("connection {}", input.id)))?;
        if input.clear_nickname {
            entry.connection.nickname = None;
        } else if let Some(n) = nickname {
            entry.connection.nickname = Some(n);
        }
        if input.clear_color {
            entry.connection.color = None;
        } else if let Some(c) = input.color {
            entry.connection.color = Some(c);
        }
        let updated = entry.connection.clone();
        persist_locked(&self.file_path, &guard)?;
        Ok(updated)
    }

    pub fn mark_used(&self, id: Uuid) -> Result<(), AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        if let Some(entry) = guard.get_mut(&id) {
            entry.connection.last_used = Some(Utc::now());
            persist_locked(&self.file_path, &guard)?;
        }
        Ok(())
    }

    /// Cache the ARM resource ID for a connection so the auto-firewall path
    /// skips subscription discovery on subsequent 40615s. Persists so the
    /// cache survives restart. No-ops if the id is unknown (deleted between
    /// discovery and caching).
    pub fn set_server_arm_id(&self, id: Uuid, arm_id: String) -> Result<(), AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        if let Some(entry) = guard.get_mut(&id) {
            entry.server_arm_id = Some(arm_id);
            persist_locked(&self.file_path, &guard)?;
        }
        Ok(())
    }

    /// Backfill `Connection.account_id` on every AAD-token entry that's
    /// currently `None`. Used at startup to migrate connections created before
    /// per-account tokens landed. Returns the number of entries touched so the
    /// caller can log the migration.
    pub fn backfill_missing_account_id(&self, account_id: &str) -> Result<usize, AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("registry mutex poisoned"))?;
        let mut touched = 0usize;
        for entry in guard.values_mut() {
            if entry.connection.auth_mode.uses_aad_bearer()
                && entry.connection.account_id.is_none()
            {
                entry.connection.account_id = Some(account_id.to_string());
                touched += 1;
            }
        }
        if touched > 0 {
            persist_locked(&self.file_path, &guard)?;
        }
        Ok(touched)
    }
}

pub struct ConnectionSnapshot {
    pub connection: Connection,
    pub password: Option<String>,
    pub trust_server_certificate: bool,
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub server_arm_id: Option<String>,
}

// ---- disk IO ----------------------------------------------------------------

// Load JSON + hydrate passwords from keychain. First-run (no file) returns an
// empty map, not an error. Corrupt JSON logs and returns empty — we'd rather
// the user re-add connections than crash on launch.
fn load_from_disk(path: &Path) -> Result<HashMap<Uuid, ConnectionEntry>, AppError> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HashMap::new());
        }
        Err(e) => {
            return Err(AppError::internal(format!(
                "read {}: {e}",
                path.display()
            )));
        }
    };

    let persisted: Vec<PersistedEntry> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(
                target: "queryben::registry::load",
                path = %path.display(),
                "connections.json is corrupt, starting empty: {err}"
            );
            return Ok(HashMap::new());
        }
    };

    let mut out = HashMap::with_capacity(persisted.len());
    for p in persisted {
        // AAD entries have no keychain password; skip the lookup entirely to
        // avoid noisy "NoEntry" warnings for the common case.
        let password = if matches!(p.connection.auth_mode, AuthMode::SqlAuth) {
            match kc_load(&p.connection.id.to_string()) {
                Ok(pw) => pw,
                Err(err) => {
                    // User denied keychain access, or the entry vanished. Keep
                    // the connection metadata around; the user can re-enter
                    // the password at reopen. Do NOT drop the entry.
                    tracing::warn!(
                        target: "queryben::registry::load",
                        id = %p.connection.id,
                        "keychain read failed: {err}"
                    );
                    None
                }
            }
        } else {
            None
        };

        let entry = ConnectionEntry {
            connection: p.connection.clone(),
            password,
            trust_server_certificate: p.trust_server_certificate,
            tenant_id: p.tenant_id,
            client_id: p.client_id,
            server_arm_id: p.server_arm_id,
        };
        out.insert(p.connection.id, entry);
    }

    Ok(out)
}

// Atomic write: dump to `connections.json.tmp` then rename over the target.
// Rename is atomic within a filesystem on all three of our OSes, so we never
// leave a truncated JSON where a reader could observe it.
fn persist_locked(
    path: &Path,
    map: &HashMap<Uuid, ConnectionEntry>,
) -> Result<(), AppError> {
    let mut entries: Vec<PersistedEntry> =
        map.values().map(PersistedEntry::from_entry).collect();
    // Stable ordering so version-controlled dev fixtures and diffs stay clean.
    entries.sort_by_key(|e| e.connection.id);

    let json = serde_json::to_vec_pretty(&entries)
        .map_err(|e| AppError::internal(format!("serialize registry: {e}")))?;

    let parent = path.parent().ok_or_else(|| {
        AppError::internal(format!("registry path has no parent: {}", path.display()))
    })?;
    let tmp = parent.join(REGISTRY_TMP);

    // Scoped so the file handle is dropped (and thus fsync'd on Drop for
    // BufWriter analogs) before the rename.
    {
        let mut file = fs::File::create(&tmp).map_err(|e| {
            AppError::internal(format!("create {}: {e}", tmp.display()))
        })?;
        file.write_all(&json).map_err(|e| {
            AppError::internal(format!("write {}: {e}", tmp.display()))
        })?;
        // Best-effort fsync; on the rare "power loss between write and rename"
        // path we'd rather have a fully-written tmp survive than a torn target.
        file.sync_all().ok();
    }

    fs::rename(&tmp, path).map_err(|e| {
        AppError::internal(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;

    Ok(())
}

// ---- keychain helpers -------------------------------------------------------
//
// Delegates to crate::adapters::keychain so the platform-specific logic (macOS
// access group + legacy default-group migration; keyring pass-through on
// Windows + Linux) lives in exactly one place.

use crate::adapters::keychain;

fn kc_store(account: &str, value: &str) -> Result<(), AppError> {
    keychain::set_password(KEYRING_SERVICE, account, value)
}

fn kc_load(account: &str) -> Result<Option<String>, AppError> {
    keychain::get_password(KEYRING_SERVICE, account)
}

fn kc_delete(account: &str) -> Result<(), AppError> {
    keychain::delete_password(KEYRING_SERVICE, account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mk_aad_entry(id: Uuid, name: &str, account_id: Option<String>) -> ConnectionEntry {
        ConnectionEntry {
            connection: Connection {
                id,
                name: name.to_string(),
                server: "srv.database.windows.net".into(),
                database: "db".into(),
                port: None,
                username: None,
                auth_mode: AuthMode::AadToken,
                created_at: Utc::now(),
                last_used: None,
                account_id,
                nickname: None,
                color: None,
            },
            password: None,
            trust_server_certificate: false,
            tenant_id: Some("tid".into()),
            client_id: Some("cid".into()),
            server_arm_id: None,
        }
    }

    fn mk_aad_interactive_entry(id: Uuid) -> ConnectionEntry {
        let mut e = mk_aad_entry(id, "aad-interactive-legacy", None);
        e.connection.auth_mode = AuthMode::AadInteractive;
        e
    }

    #[test]
    fn backfill_covers_aad_interactive_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let id = Uuid::new_v4();
        reg.insert(mk_aad_interactive_entry(id)).expect("insert");
        let touched = reg
            .backfill_missing_account_id("oid.tid")
            .expect("backfill");
        assert_eq!(touched, 1);
        let snap = reg.snapshot(id).expect("snap");
        assert_eq!(snap.connection.account_id.as_deref(), Some("oid.tid"));
    }

    fn mk_sql_entry(id: Uuid) -> ConnectionEntry {
        ConnectionEntry {
            connection: Connection {
                id,
                name: "sql-auth".into(),
                server: "srv".into(),
                database: "db".into(),
                port: None,
                username: Some("sa".into()),
                auth_mode: AuthMode::SqlAuth,
                created_at: Utc::now(),
                last_used: None,
                account_id: None,
                nickname: None,
                color: None,
            },
            password: None,
            trust_server_certificate: false,
            tenant_id: None,
            client_id: None,
            server_arm_id: None,
        }
    }

    #[test]
    fn uses_aad_bearer_covers_token_and_interactive() {
        assert!(AuthMode::AadToken.uses_aad_bearer());
        assert!(AuthMode::AadInteractive.uses_aad_bearer());
        assert!(!AuthMode::SqlAuth.uses_aad_bearer());
        assert!(!AuthMode::AadPassword.uses_aad_bearer());
        assert!(!AuthMode::AadManagedIdentity.uses_aad_bearer());
    }

    #[test]
    fn backfill_populates_none_account_id_on_aad_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        reg.insert(mk_aad_entry(a, "aad-legacy-a", None)).expect("a");
        reg.insert(mk_aad_entry(b, "aad-legacy-b", None)).expect("b");

        let touched = reg
            .backfill_missing_account_id("oid.tid")
            .expect("backfill");
        assert_eq!(touched, 2);

        let snap_a = reg.snapshot(a).expect("snap a");
        let snap_b = reg.snapshot(b).expect("snap b");
        assert_eq!(snap_a.connection.account_id.as_deref(), Some("oid.tid"));
        assert_eq!(snap_b.connection.account_id.as_deref(), Some("oid.tid"));
    }

    #[test]
    fn backfill_leaves_existing_account_id_alone() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let id = Uuid::new_v4();
        reg.insert(mk_aad_entry(id, "already-bound", Some("keep.me".into())))
            .expect("insert");

        let touched = reg
            .backfill_missing_account_id("do.not.overwrite")
            .expect("backfill");
        assert_eq!(touched, 0);
        let snap = reg.snapshot(id).expect("snap");
        assert_eq!(snap.connection.account_id.as_deref(), Some("keep.me"));
    }

    #[test]
    fn backfill_skips_sql_auth_entries() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let id = Uuid::new_v4();
        reg.insert(mk_sql_entry(id)).expect("insert");

        let touched = reg
            .backfill_missing_account_id("oid.tid")
            .expect("backfill");
        assert_eq!(touched, 0);
        let snap = reg.snapshot(id).expect("snap");
        assert!(snap.connection.account_id.is_none());
    }

    #[test]
    fn normalize_nickname_trims_and_caps() {
        assert!(matches!(normalize_nickname(None), Ok(None)));
        assert!(matches!(normalize_nickname(Some("  ".into())), Ok(None)));
        assert_eq!(
            normalize_nickname(Some("  Prod  ".into())).unwrap(),
            Some("Prod".to_string()),
        );
        let too_long = "x".repeat(NICKNAME_MAX_LEN + 1);
        assert!(matches!(
            normalize_nickname(Some(too_long)),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn display_name_prefers_nickname_over_hostname() {
        let mut conn = Connection {
            id: Uuid::new_v4(),
            name: "n".into(),
            server: "srv.database.windows.net".into(),
            database: "db".into(),
            port: None,
            username: None,
            auth_mode: AuthMode::AadToken,
            created_at: Utc::now(),
            last_used: None,
            account_id: None,
            nickname: None,
            color: None,
        };
        assert_eq!(conn.display_name(), "srv.database.windows.net");
        conn.nickname = Some("Prod East".into());
        assert_eq!(conn.display_name(), "Prod East");
    }

    #[test]
    fn update_labels_sets_and_clears_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let id = Uuid::new_v4();
        reg.insert(mk_aad_entry(id, "seed", None)).expect("insert");

        let updated = reg
            .update_labels(UpdateConnectionInput {
                id,
                nickname: Some("Prod · East".into()),
                color: Some(ConnectionColor::Amber),
                clear_nickname: false,
                clear_color: false,
            })
            .expect("update");
        assert_eq!(updated.nickname.as_deref(), Some("Prod · East"));
        assert_eq!(updated.color, Some(ConnectionColor::Amber));

        let cleared = reg
            .update_labels(UpdateConnectionInput {
                id,
                nickname: None,
                color: None,
                clear_nickname: true,
                clear_color: true,
            })
            .expect("clear");
        assert!(cleared.nickname.is_none());
        assert!(cleared.color.is_none());
    }

    #[test]
    fn update_labels_rejects_overflow() {
        let tmp = TempDir::new().expect("tempdir");
        let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
        let id = Uuid::new_v4();
        reg.insert(mk_aad_entry(id, "seed", None)).expect("insert");
        let err = reg
            .update_labels(UpdateConnectionInput {
                id,
                nickname: Some("x".repeat(NICKNAME_MAX_LEN + 5)),
                color: None,
                clear_nickname: false,
                clear_color: false,
            })
            .expect_err("expected validation error");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn backfill_persists_across_reload() {
        let tmp = TempDir::new().expect("tempdir");
        let id = Uuid::new_v4();
        {
            let reg = ConnectionRegistry::new(tmp.path()).expect("registry");
            reg.insert(mk_aad_entry(id, "legacy", None)).expect("insert");
            let touched = reg
                .backfill_missing_account_id("oid.tid")
                .expect("backfill");
            assert_eq!(touched, 1);
        }
        // Reload from disk.
        let reg2 = ConnectionRegistry::new(tmp.path()).expect("reload");
        let snap = reg2.snapshot(id).expect("snap");
        assert_eq!(snap.connection.account_id.as_deref(), Some("oid.tid"));
    }
}
