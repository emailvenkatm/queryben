//! Integration tests for `import_from_ads`. Every test operates entirely
//! inside a tempdir rooted at `QUERYBEN_ADS_ROOT` — the real ADS install and
//! the real macOS keychain are never touched.

use serial_test::serial;
use tempfile::TempDir;

use queryben_lib::core::connection::ConnectionRegistry;
use queryben_lib::adapters::ads_bridge::{
    self, ENV_ADS_ROOT_OVERRIDE, ENV_QB_SNIPPETS_PATH_OVERRIDE,
};

const SETTINGS_JSON: &str = r#"{
    "datasource.connections": [
        {
            "options": {
                "server": "prod.database.windows.net",
                "database": "app",
                "authenticationType": "AzureMFA",
                "user": "Alice - alice@contoso.com",
                "azureTenantId": "tid-1",
                "azureAccount": "oid.tid-1",
                "azureResourceId": "/subscriptions/xxx/rgs/rg/providers/Microsoft.Sql/servers/prod"
            },
            "providerName": "MSSQL",
            "id": "conn-1"
        },
        {
            "options": {
                "server": "staging.database.windows.net",
                "database": "app",
                "authenticationType": "AzureMFA",
                "user": "Alice - alice@contoso.com",
                "azureTenantId": "tid-1",
                "azureAccount": "oid.tid-1"
            },
            "providerName": "MSSQL",
            "id": "conn-2"
        },
        {
            "options": {
                "server": "localhost,1433",
                "database": "master",
                "authenticationType": "SqlLogin",
                "user": "sa"
            },
            "providerName": "MSSQL",
            "id": "conn-3"
        }
    ]
}"#;

struct Sandbox {
    _tmp: TempDir,
    user_dir: std::path::PathBuf,
    snippets_target: std::path::PathBuf,
    registry_dir: std::path::PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("mk tempdir");
        let user_dir = tmp.path().join("User");
        let snippets_target = tmp.path().join("qb-snippets.json");
        let registry_dir = tmp.path().join("qb");
        std::fs::create_dir_all(&user_dir).expect("mk user dir");
        std::fs::create_dir_all(&registry_dir).expect("mk registry dir");
        std::env::set_var(ENV_ADS_ROOT_OVERRIDE, tmp.path());
        std::env::set_var(ENV_QB_SNIPPETS_PATH_OVERRIDE, &snippets_target);
        Self {
            _tmp: tmp,
            user_dir,
            snippets_target,
            registry_dir,
        }
    }
    fn write_settings(&self, s: &str) {
        std::fs::write(self.user_dir.join("settings.json"), s).expect("write settings");
    }
    fn write_snippet(&self, name: &str, s: &str) {
        let dir = self.user_dir.join("snippets");
        std::fs::create_dir_all(&dir).expect("mkdirs");
        std::fs::write(dir.join(name), s).expect("write snippet");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::env::remove_var(ENV_ADS_ROOT_OVERRIDE);
        std::env::remove_var(ENV_QB_SNIPPETS_PATH_OVERRIDE);
    }
}

#[tokio::test]
#[serial]
async fn imports_three_connections_into_registry() {
    let s = Sandbox::new();
    s.write_settings(SETTINGS_JSON);
    let registry = ConnectionRegistry::new(&s.registry_dir).expect("registry");

    let summary = ads_bridge::import_from_ads(&registry).await.expect("import");
    assert_eq!(summary.connections_imported, 3);
    assert_eq!(summary.accounts_imported, 0, "sandboxed prime must not touch keychain");
    let list = registry.list().expect("list");
    assert_eq!(list.len(), 3);
    assert!(list.iter().any(|c| c.server == "prod.database.windows.net"));
    assert!(list.iter().any(|c| c.server == "staging.database.windows.net"));
    assert!(list.iter().any(|c| c.server == "localhost,1433"));
}

#[tokio::test]
#[serial]
async fn second_import_is_idempotent_no_duplicates() {
    let s = Sandbox::new();
    s.write_settings(SETTINGS_JSON);
    let registry = ConnectionRegistry::new(&s.registry_dir).expect("registry");

    let first = ads_bridge::import_from_ads(&registry).await.expect("import 1");
    let second = ads_bridge::import_from_ads(&registry).await.expect("import 2");
    assert_eq!(first.connections_imported, 3);
    assert_eq!(second.connections_imported, 0);
    let list = registry.list().expect("list");
    assert_eq!(list.len(), 3);
}

#[tokio::test]
#[serial]
async fn snippets_get_copied_and_deduped() {
    let s = Sandbox::new();
    s.write_settings(SETTINGS_JSON);
    s.write_snippet(
        "mssql.json",
        r#"{ "top10": { "prefix": "top10", "body": "SELECT TOP 10 * FROM ${1:t}" } }"#,
    );
    let registry = ConnectionRegistry::new(&s.registry_dir).expect("registry");

    let r1 = ads_bridge::import_from_ads(&registry).await.expect("import 1");
    assert_eq!(r1.snippets_imported, 1);
    assert!(s.snippets_target.exists());

    let r2 = ads_bridge::import_from_ads(&registry).await.expect("import 2");
    assert_eq!(r2.snippets_imported, 0, "existing snippet must not re-add");

    let contents = std::fs::read_to_string(&s.snippets_target).expect("read");
    let arr: Vec<serde_json::Value> = serde_json::from_str(&contents).expect("parse");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("name").and_then(|v| v.as_str()), Some("top10"));
}

#[tokio::test]
#[serial]
async fn missing_ads_returns_zero_import_summary() {
    let s = Sandbox::new();
    // Don't write settings.
    let registry = ConnectionRegistry::new(&s.registry_dir).expect("registry");
    let summary = ads_bridge::import_from_ads(&registry).await.expect("import");
    assert_eq!(summary.connections_imported, 0);
    assert_eq!(summary.snippets_imported, 0);
    let list = registry.list().expect("list");
    assert!(list.is_empty());
}
