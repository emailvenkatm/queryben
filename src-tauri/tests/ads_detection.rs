//! Integration tests for `detect_ads_installation`. Every test operates
//! entirely inside a tempdir rooted at `QUERYBEN_ADS_ROOT` — the real ADS
//! install and the real macOS keychain are never touched.

use serial_test::serial;
use tempfile::TempDir;

use queryben_lib::adapters::ads_bridge::{self, ENV_ADS_ROOT_OVERRIDE};

const SETTINGS_WITH_THREE_CONNECTIONS: &str = r#"{
    "datasource.connections": [
        {
            "options": {
                "server": "prod-contoso.database.windows.net",
                "database": "app",
                "authenticationType": "AzureMFA",
                "user": "Alice Anderson - alice@contoso.com",
                "azureTenantId": "11111111-1111-1111-1111-111111111111",
                "azureAccount": "aaaa.11111111-1111-1111-1111-111111111111",
                "azureResourceId": "/subscriptions/xxx/resourceGroups/rg/providers/Microsoft.Sql/servers/prod-contoso"
            },
            "providerName": "MSSQL",
            "id": "conn-1"
        },
        {
            "options": {
                "server": "staging.database.windows.net",
                "database": "app",
                "authenticationType": "AzureMFA",
                "user": "Alice Anderson - alice@contoso.com"
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
}

impl Sandbox {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("mk tempdir");
        let user_dir = tmp.path().join("User");
        std::fs::create_dir_all(&user_dir).expect("mk user dir");
        std::env::set_var(ENV_ADS_ROOT_OVERRIDE, tmp.path());
        Self {
            _tmp: tmp,
            user_dir,
        }
    }
    fn write_settings(&self, contents: &str) {
        std::fs::write(self.user_dir.join("settings.json"), contents).expect("write settings");
    }
    fn write_snippet(&self, name: &str, contents: &str) {
        let dir = self.user_dir.join("snippets");
        std::fs::create_dir_all(&dir).expect("mkdirs");
        std::fs::write(dir.join(name), contents).expect("write snippet");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        std::env::remove_var(ENV_ADS_ROOT_OVERRIDE);
    }
}

#[test]
#[serial]
fn missing_ads_returns_none() {
    let tmp = tempfile::tempdir().expect("mk tempdir");
    std::env::set_var(ENV_ADS_ROOT_OVERRIDE, tmp.path());
    let result = ads_bridge::detect_ads_installation();
    std::env::remove_var(ENV_ADS_ROOT_OVERRIDE);
    assert!(result.is_none());
}

#[test]
#[serial]
fn detects_three_connections_with_msal_email() {
    let s = Sandbox::new();
    s.write_settings(SETTINGS_WITH_THREE_CONNECTIONS);
    s.write_snippet(
        "mssql.json",
        r#"{ "top10": { "prefix": "top10", "body": "SELECT TOP 10 * FROM ${1:table}" } }"#,
    );
    s.write_snippet(
        "sql.code-snippets",
        r#"{ "count-all": { "prefix": "cntall", "body": "SELECT COUNT(*) FROM ${1:table}" } }"#,
    );

    let summary = ads_bridge::detect_ads_installation().expect("some");
    assert_eq!(summary.connection_count, 3);
    assert_eq!(summary.msal_account_email.as_deref(), Some("alice@contoso.com"));
    assert_eq!(summary.snippet_count, 2);
    assert!(summary.install_path.ends_with("User"));
}

#[test]
#[serial]
fn malformed_settings_returns_none_no_panic() {
    let s = Sandbox::new();
    s.write_settings("{ not valid json at all");
    let result = ads_bridge::detect_ads_installation();
    assert!(result.is_none(), "corrupt settings must return None");
}

#[test]
#[serial]
fn empty_settings_returns_zero_counts() {
    let s = Sandbox::new();
    s.write_settings("{}");
    let summary = ads_bridge::detect_ads_installation().expect("some");
    assert_eq!(summary.connection_count, 0);
    assert_eq!(summary.snippet_count, 0);
    assert!(summary.msal_account_email.is_none());
}
