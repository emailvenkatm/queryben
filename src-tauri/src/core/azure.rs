//! Azure resource shapes from ARM. Framework-agnostic; azure_rest parses
//! JSON into these.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AzureSubscription {
    pub id: String,
    pub subscription_id: String,
    pub display_name: String,
    pub tenant_id: String,
    // `Enabled` | `Disabled` | `Warned` | `PastDue` | `Deleted`, verbatim.
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AzureSqlServer {
    pub id: String,
    pub name: String,
    pub resource_group: String,
    pub location: String,
    // e.g. `myserver.database.windows.net`; this is the tiberius target.
    pub fully_qualified_domain_name: String,
    pub administrator_login: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AzureSqlDatabase {
    pub id: String,
    pub name: String,
    pub server_name: String,
    pub location: String,
    pub sku_tier: Option<String>,
    pub sku_name: Option<String>,
    pub status: Option<String>,
    pub collation: Option<String>,
    pub creation_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AzureConnectInput {
    pub display_name: String,
    pub server_fqdn: String,
    pub database: String,
    // Needed so the connect path can auto-add a firewall rule on 40615.
    pub server_id: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub color: Option<crate::core::connection::ConnectionColor>,
}
