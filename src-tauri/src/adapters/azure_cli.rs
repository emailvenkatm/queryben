//! Shared Azure CLI token cache.
//!
//! `az account get-access-token --resource <resource> --output json` is what
//! Azure Data Studio, PowerShell's `Get-AzAccessToken`, and every MS
//! first-party tool piggybacks on when the user is already `az login`-ed.
//! It reads from `~/.azure/msal_token_cache.json` (or the legacy
//! `accessTokens.json`), silently trades a cached refresh token when needed,
//! and prints a JSON blob with an `accessToken` field.
//!
//! We probe this BEFORE falling back to our own keychain-cached refresh token
//! so that anyone with `az login` already done never sees QueryBen's browser
//! sign-in dialog — including immediately after the OS keychain is cleared
//! (dev churn, reinstall, keychain reset).
//!
//! Failure modes we handle silently (return `None`, never panic, never log at
//! warn+ during normal launch):
//!   - `az` not on `$PATH` (dev machine without Azure CLI installed)
//!   - `az account get-access-token` non-zero exit (not signed in, tenant
//!     mismatch, subscription disabled, network hiccup on the CLI side)
//!   - Malformed / truncated JSON
//!   - Hang: capped at `CLI_TIMEOUT` so a wedged `az` never freezes the app

use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

// 5s is comfortably above the ~800ms cold-start of `az` on macOS while still
// short enough that a wedged CLI can't stall a UI-triggered probe.
const CLI_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
struct AzCliToken {
    #[serde(rename = "accessToken")]
    access_token: String,
}

/// Full token payload from `az account get-access-token`. `expires_on` is the
/// Unix epoch (seconds); `expiresOn` (RFC-ish local time) is discarded. Only
/// used by callers that want to seed our in-memory cache with a real expiry.
#[derive(Deserialize)]
pub struct AzCliTokenFull {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// Seconds since Unix epoch. Present since az CLI 2.54; older CLIs only
    /// have the local-time `expiresOn` string, which we don't parse.
    #[serde(rename = "expires_on", default)]
    pub expires_on: Option<i64>,
}

/// Same probe as `get_access_token_via_cli` but returns the epoch expiry
/// alongside the token so callers can seed a proper cache TTL.
pub async fn get_access_token_full(resource: &str) -> Option<AzCliTokenFull> {
    let normalized = resource_from_scope(resource);

    let child = Command::new("az")
        .args([
            "account",
            "get-access-token",
            "--resource",
            &normalized,
            "--output",
            "json",
        ])
        .kill_on_drop(true)
        .output();

    let output = match timeout(CLI_TIMEOUT, child).await {
        Ok(Ok(o)) => o,
        _ => return None,
    };

    if !output.status.success() {
        return None;
    }

    let parsed: AzCliTokenFull = serde_json::from_slice(&output.stdout).ok()?;
    if parsed.access_token.is_empty() {
        return None;
    }
    Some(parsed)
}

/// Normalize a scope string into the `--resource` value `az` expects.
///
/// OAuth callers pass scopes like `https://database.windows.net/.default`;
/// the CLI wants just the resource: `https://database.windows.net/`.
fn resource_from_scope(scope: &str) -> String {
    let trimmed = scope.trim_end_matches("/.default").trim_end_matches(".default");
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
}

/// Ask the shared Azure CLI cache for an access token scoped to `resource`
/// (or a `.default`-suffixed scope — we normalize).
///
/// Returns `None` if `az` isn't installed, isn't signed in, exits non-zero,
/// emits malformed JSON, or hangs longer than `CLI_TIMEOUT`. Never panics.
pub async fn get_access_token_via_cli(resource: &str) -> Option<String> {
    let normalized = resource_from_scope(resource);

    let child = Command::new("az")
        .args([
            "account",
            "get-access-token",
            "--resource",
            &normalized,
            "--output",
            "json",
        ])
        .kill_on_drop(true)
        .output();

    let output = match timeout(CLI_TIMEOUT, child).await {
        // Successful spawn + wait within the budget.
        Ok(Ok(o)) => o,
        // Spawn failed. Overwhelmingly this is `az` missing from PATH — silent
        // by design; we don't want a scary warning on every launch for users
        // without the CLI installed.
        Ok(Err(_)) => return None,
        // Timed out. `kill_on_drop` above SIGKILLs the child when this future
        // is dropped, so we don't leave a zombie behind.
        Err(_) => {
            tracing::debug!(
                target: "queryben::azure_cli",
                "az account get-access-token exceeded {}s timeout",
                CLI_TIMEOUT.as_secs()
            );
            return None;
        }
    };

    if !output.status.success() {
        // Common non-zero cases: "Please run 'az login'", tenant mismatch,
        // "no subscriptions found". All expected — debug-level only.
        tracing::debug!(
            target: "queryben::azure_cli",
            status = ?output.status.code(),
            "az get-access-token exited non-zero"
        );
        return None;
    }

    let parsed: AzCliToken = match serde_json::from_slice(&output.stdout) {
        Ok(t) => t,
        Err(err) => {
            tracing::debug!(
                target: "queryben::azure_cli",
                %err,
                "az get-access-token JSON parse failed"
            );
            return None;
        }
    };

    if parsed.access_token.is_empty() {
        return None;
    }

    Some(parsed.access_token)
}

/// Cheap probe: is the user signed in to `az`? Only checks the exit code of
/// `az account show`; we don't parse the JSON. Used by callers that want to
/// decide up-front whether to bother with the CLI path.
///
/// Returns `false` for any failure mode (not installed, not signed in, hang).
pub async fn is_signed_in() -> bool {
    let child = Command::new("az")
        .args(["account", "show"])
        .kill_on_drop(true)
        .output();

    match timeout(CLI_TIMEOUT, child).await {
        Ok(Ok(o)) => o.status.success(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::resource_from_scope;

    #[test]
    fn strips_default_suffix() {
        assert_eq!(
            resource_from_scope("https://database.windows.net/.default"),
            "https://database.windows.net/"
        );
        assert_eq!(
            resource_from_scope("https://management.azure.com/.default"),
            "https://management.azure.com/"
        );
    }

    #[test]
    fn adds_trailing_slash() {
        assert_eq!(
            resource_from_scope("https://database.windows.net"),
            "https://database.windows.net/"
        );
    }

    #[test]
    fn leaves_bare_resource_alone() {
        assert_eq!(
            resource_from_scope("https://database.windows.net/"),
            "https://database.windows.net/"
        );
    }
}
