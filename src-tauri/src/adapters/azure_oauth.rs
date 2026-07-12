//! Azure AD sign-in: system browser + loopback callback + PKCE.
//!
//! MSAL-in-webview does not work in Tauri. WKWebView rejects the cross-origin
//! popup polling that MSAL depends on, and the redirect flow can't be
//! intercepted the way a real browser would. Microsoft's guidance for desktop
//! (aka.ms/msal-net-web-browsers): open the system browser, listen on
//! localhost, use PKCE S256, exchange code for tokens, stash the refresh
//! token in the OS keychain.
//!
//! One refresh token minted with `offline_access` can trade for access tokens
//! against any resource the user has already consented to (management,
//! database.windows.net, Graph), so we only run the browser dance once.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::ShellExt;
use tiny_http::{Header, Response, Server};
use url::Url;

use crate::error::AppError;
use crate::adapters::azure_accounts::{self, AccountRegistryEntry};
use crate::adapters::token_file_cache::{self, PersistedAccount};

// ---- constants ----------------------------------------------------------------

// Azure requires each redirect URI be registered exactly, so this has to be
// a fixed port. If 8400 is in use, sign-in fails with a clear error.
const CALLBACK_PORT: u16 = 8400;
const CALLBACK_PATH: &str = "/callback";
const REDIRECT_URI: &str = "http://localhost:8400/callback";

const AUTHORITY_BASE: &str = "https://login.microsoftonline.com";

// Empty tenant falls back to /organizations. Personal-MSA guest users need a
// real tenant GUID pinned; /common and /organizations won't disambiguate them.
fn authority(tenant_id: &str) -> String {
    if tenant_id.is_empty() {
        format!("{AUTHORITY_BASE}/organizations")
    } else {
        format!("{AUTHORITY_BASE}/{tenant_id}")
    }
}
const KEYRING_SERVICE: &str = "com.queryben.azure";
// Legacy single-account keys — kept as read-only fallback for the migration
// path. New writes go through `refresh_key_for` / `account_info_key_for`,
// which suffix with the MSAL home_account_id.
const KEYRING_REFRESH_ACCOUNT: &str = "refresh_token";
const KEYRING_ACCOUNT_INFO: &str = "account_info";

fn refresh_key_for(account_id: &str) -> String {
    format!("refresh_token::{account_id}")
}

fn account_info_key_for(account_id: &str) -> String {
    format!("account_info::{account_id}")
}

// Refresh cached tokens 5 min before Azure's typical 3600s expiry.
const REFRESH_GRACE_SECONDS: i64 = 300;

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

// ---- public types -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AzureAccount {
    // `oid` claim, falling back to `sub` for personal MSA.
    pub id: String,
    // `preferred_username` or `email` from the id_token.
    pub username: String,
    // Resolved from /common, so later token requests skip tenant discovery.
    pub tenant_id: String,
    // MSAL-style `<oid>.<tid>`, kept for a possible MSAL-native migration later.
    pub home_account_id: String,
    pub name: Option<String>,
}

// ---- in-memory token cache ----------------------------------------------------

#[derive(Default)]
struct TokenCacheInner {
    tokens: HashMap<String, (String, DateTime<Utc>)>,
    account: Option<AzureAccount>,
}

#[derive(Default)]
pub struct TokenCache {
    inner: Mutex<TokenCacheInner>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, scope: &str) -> Result<Option<String>, AppError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("token cache mutex poisoned"))?;
        let Some((token, expires_at)) = guard.tokens.get(scope) else {
            return Ok(None);
        };
        let now = Utc::now();
        if (*expires_at - now).num_seconds() > REFRESH_GRACE_SECONDS {
            Ok(Some(token.clone()))
        } else {
            Ok(None)
        }
    }

    fn put(&self, scope: &str, token: String, expires_at: DateTime<Utc>) -> Result<(), AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("token cache mutex poisoned"))?;
        guard.tokens.insert(scope.to_string(), (token, expires_at));
        Ok(())
    }

    fn clear_all(&self) -> Result<(), AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("token cache mutex poisoned"))?;
        guard.tokens.clear();
        guard.account = None;
        Ok(())
    }

    fn set_account(&self, account: Option<AzureAccount>) -> Result<(), AppError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("token cache mutex poisoned"))?;
        guard.account = account;
        Ok(())
    }

    fn get_account(&self) -> Result<Option<AzureAccount>, AppError> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| AppError::internal("token cache mutex poisoned"))?;
        Ok(guard.account.clone())
    }
}

// ---- keychain helpers ---------------------------------------------------------
//
// Thin adapters over crate::adapters::keychain so the module still reads as
// `kc_store` / `kc_load` / `kc_delete` locally. See infra::keychain for the
// macOS access-group + legacy migration story.

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

// ---- PKCE ---------------------------------------------------------------------

// 96 bytes -> 128-char base64url-nopad verifier. Azure caps at 128, this pins
// us to the max entropy that still validates.
fn make_pkce() -> (String, String) {
    let mut raw = [0u8; 96];
    thread_rng().fill(&mut raw[..]);
    let verifier = URL_SAFE_NO_PAD.encode(raw);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn make_state() -> String {
    let mut raw = [0u8; 32];
    thread_rng().fill(&mut raw[..]);
    URL_SAFE_NO_PAD.encode(raw)
}

// ---- authorize URL ------------------------------------------------------------

// Azure v2 rejects multiple resource `.default` scopes in one interactive
// request ("static scope limit exceeded"). Ask for management up front; mint
// the sqldb token later by trading the refresh_token. Admin consent already
// covers both APIs so the resource swap is silent.
const SCOPES: &str = "https://management.azure.com/.default \
                      offline_access \
                      openid \
                      profile";

fn build_authorize_url(
    tenant_id: &str,
    client_id: &str,
    code_challenge: &str,
    state: &str,
) -> Result<String, AppError> {
    let mut url = Url::parse(&format!("{}/oauth2/v2.0/authorize", authority(tenant_id)))
        .map_err(|e| AppError::internal(format!("authorize url: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("prompt", "select_account");
    Ok(url.into())
}

// ---- loopback listener --------------------------------------------------------

fn success_page() -> &'static str {
    r#"<!doctype html><html><head><meta charset="utf-8"><title>QueryBen</title>
<style>body{font-family:-apple-system,BlinkMacSystemFont,sans-serif;background:#f8f5ee;color:#1a1a1a;
margin:0;display:flex;align-items:center;justify-content:center;min-height:100vh}
.card{background:#fff;border:1px solid #e5ded1;border-radius:12px;padding:32px 40px;
box-shadow:0 1px 3px rgba(0,0,0,.04);text-align:center;max-width:400px}
h1{color:#2A5751;font-size:20px;margin:0 0 8px}
p{color:#555;font-size:14px;margin:0;line-height:1.5}</style></head>
<body><div class="card"><h1>Signed in</h1><p>You can close this window and return to QueryBen.</p></div></body></html>"#
}

fn error_page(msg: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>QueryBen: sign-in failed</title>
<style>body{{font-family:-apple-system,BlinkMacSystemFont,sans-serif;background:#f8f5ee;color:#1a1a1a;
margin:0;display:flex;align-items:center;justify-content:center;min-height:100vh}}
.card{{background:#fff;border:1px solid #e5ded1;border-radius:12px;padding:32px 40px;
box-shadow:0 1px 3px rgba(0,0,0,.04);text-align:center;max-width:480px}}
h1{{color:#b45309;font-size:20px;margin:0 0 8px}}
p{{color:#555;font-size:14px;margin:0;line-height:1.5}}
code{{background:#f4efe5;padding:2px 6px;border-radius:4px;font-size:12px}}</style></head>
<body><div class="card"><h1>Sign-in failed</h1><p>{}</p></div></body></html>"#,
        html_escape(msg)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct CallbackResult {
    code: String,
}

// Blocks until the browser hits /callback or SIGN_IN_TIMEOUT fires. Runs
// tiny_http inline because the caller is already on spawn_blocking.
fn await_callback(state_expected: &str) -> Result<CallbackResult, AppError> {
    let server = Server::http(format!("127.0.0.1:{CALLBACK_PORT}"))
        .map_err(|e| AppError::internal(format!("loopback bind failed on {CALLBACK_PORT}: {e}. Close whatever's holding the port and retry.")))?;

    let deadline = Instant::now() + SIGN_IN_TIMEOUT;

    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| AppError::AuthFailed("sign-in timed out after 5 minutes".into()))?;

        let request = match server.recv_timeout(remaining) {
            Ok(Some(req)) => req,
            Ok(None) => {
                return Err(AppError::AuthFailed(
                    "sign-in timed out after 5 minutes".into(),
                ));
            }
            Err(e) => {
                return Err(AppError::internal(format!("loopback recv: {e}")));
            }
        };

        // Anything not on /callback (favicon etc) gets a 404 and we loop.
        let url = request.url().to_string();
        if !url.starts_with(CALLBACK_PATH) {
            let _ = request.respond(Response::from_string("not found").with_status_code(404));
            continue;
        }

        // Url::parse needs a base for the relative URL.
        let full = format!("http://localhost{url}");
        let parsed = match Url::parse(&full) {
            Ok(u) => u,
            Err(e) => {
                let body = error_page(&format!("bad callback URL: {e}"));
                let _ = respond_html(request, 400, body);
                continue;
            }
        };

        let params: HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();

        if let Some(err) = params.get("error") {
            let desc = params
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| err.clone());
            let body = error_page(&desc);
            let _ = respond_html(request, 400, body);
            return Err(AppError::AuthFailed(format!("azure returned error: {desc}")));
        }

        let state_got = params.get("state").cloned().unwrap_or_default();
        if state_got != state_expected {
            let body = error_page("state mismatch, possible CSRF; request rejected");
            let _ = respond_html(request, 400, body);
            return Err(AppError::AuthFailed(
                "OAuth state mismatch (possible CSRF)".into(),
            ));
        }

        let Some(code) = params.get("code").cloned() else {
            let body = error_page("callback missing 'code' parameter");
            let _ = respond_html(request, 400, body);
            return Err(AppError::AuthFailed(
                "callback missing 'code' parameter".into(),
            ));
        };

        let _ = respond_html(request, 200, success_page().to_string());
        return Ok(CallbackResult { code });
    }
}

fn respond_html(
    req: tiny_http::Request,
    status: u16,
    body: String,
) -> Result<(), std::io::Error> {
    let len = body.len();
    // Header::from_bytes can't fail on static ASCII, but the crate-wide lint
    // bans unwrap, so we skip the header on the None branch. Browsers sniff.
    let mut headers: Vec<Header> = Vec::new();
    if let Ok(h) =
        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
    {
        headers.push(h);
    }
    let response = Response::new(
        tiny_http::StatusCode(status),
        headers,
        Cursor::new(body.into_bytes()),
        Some(len),
        None,
    );
    req.respond(response)
}

// ---- token endpoint -----------------------------------------------------------

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    // Azure rotates the refresh token every ~24h; always store the latest.
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: i64,
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

async fn post_token_form(
    tenant_id: &str,
    form: &[(&str, &str)],
) -> Result<TokenResponse, AppError> {
    let client = reqwest::Client::builder()
        .user_agent("QueryBen/0.1.0")
        .build()
        .map_err(|e| AppError::internal(format!("http client: {e}")))?;

    let resp = client
        .post(format!("{}/oauth2/v2.0/token", authority(tenant_id)))
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .unwrap_or_else(|_| "<no body>".to_string());

    if !status.is_success() {
        if let Ok(err) = serde_json::from_str::<TokenErrorResponse>(&text) {
            let desc = err.error_description.unwrap_or_default();
            return Err(AppError::AuthFailed(format!(
                "token endpoint {status}: {}: {}",
                err.error, desc
            )));
        }
        return Err(AppError::AuthFailed(format!(
            "token endpoint {status}: {text}"
        )));
    }

    serde_json::from_str::<TokenResponse>(&text)
        .map_err(|e| AppError::internal(format!("token response decode: {e}; body: {text}")))
}

// ---- id_token parsing ---------------------------------------------------------

#[derive(Deserialize)]
struct IdTokenClaims {
    oid: Option<String>,
    sub: Option<String>,
    tid: Option<String>,
    preferred_username: Option<String>,
    email: Option<String>,
    name: Option<String>,
}

// Decode-only. Azure just handed us this over TLS from its own token
// endpoint, so we trust the payload without verifying the signature.
fn parse_id_token(id_token: &str) -> Result<AzureAccount, AppError> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() < 2 {
        return Err(AppError::AuthFailed("id_token: not a JWT".into()));
    }
    let payload_b64 = parts[1];
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| AppError::AuthFailed(format!("id_token base64 decode: {e}")))?;
    let claims: IdTokenClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|e| AppError::AuthFailed(format!("id_token claims decode: {e}")))?;

    let id = claims
        .oid
        .clone()
        .or_else(|| claims.sub.clone())
        .ok_or_else(|| AppError::AuthFailed("id_token missing oid/sub".into()))?;
    let tenant_id = claims
        .tid
        .clone()
        .ok_or_else(|| AppError::AuthFailed("id_token missing tid".into()))?;
    let username = claims
        .preferred_username
        .clone()
        .or_else(|| claims.email.clone())
        .unwrap_or_default();
    let home_account_id = format!("{}.{}", id, tenant_id);

    Ok(AzureAccount {
        id,
        username,
        tenant_id,
        home_account_id,
        name: claims.name,
    })
}

// ---- public API ---------------------------------------------------------------

/// Interactive sign-in. Opens the system browser, waits for the callback,
/// stores the refresh token in the OS keychain, and seeds the mgmt access
/// token into the in-memory cache so the first REST call is instant.
pub async fn sign_in<R: Runtime>(
    app: &AppHandle<R>,
    cache: &TokenCache,
    tenant_id: &str,
    client_id: &str,
) -> Result<AzureAccount, AppError> {
    let (verifier, challenge) = make_pkce();
    let state = make_state();
    let authorize_url = build_authorize_url(tenant_id, client_id, &challenge, &state)?;

    // tauri-plugin-shell wraps macOS `open` / Windows `start` / Linux `xdg-open`.
    // TODO: migrate to tauri-plugin-opener before Tauri 2.5 removes shell::open.
    #[allow(deprecated)]
    app.shell()
        .open(authorize_url, None)
        .map_err(|e| AppError::internal(format!("system browser open failed: {e}")))?;

    // spawn_blocking so the loopback listener doesn't starve tokio.
    let state_moved = state.clone();
    let callback = tokio::task::spawn_blocking(move || await_callback(&state_moved))
        .await
        .map_err(|e| AppError::internal(format!("join loopback task: {e}")))??;

    // Trade code for tokens. Ask for the mgmt scope so we can cache the
    // access token before the first REST call.
    let scope_for_initial = "https://management.azure.com/.default offline_access openid profile";
    let form = [
        ("client_id", client_id),
        ("scope", scope_for_initial),
        ("code", callback.code.as_str()),
        ("redirect_uri", REDIRECT_URI),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier.as_str()),
    ];
    let token = post_token_form(tenant_id, &form).await?;

    let id_token = token
        .id_token
        .as_deref()
        .ok_or_else(|| AppError::AuthFailed("token response missing id_token".into()))?;
    let account = parse_id_token(id_token)?;

    let refresh_token = token
        .refresh_token
        .clone()
        .ok_or_else(|| AppError::AuthFailed(
            "token response missing refresh_token (offline_access not in scope?)".into(),
        ))?;

    // Per-account keychain slots — one refresh token per (tenant, client,
    // home_account_id) so multi-account sign-in doesn't overwrite prior tokens.
    let account_id = account.home_account_id.clone();
    kc_store(&refresh_key_for(&account_id), &refresh_token)?;
    let account_json = serde_json::to_string(&account)
        .map_err(|e| AppError::internal(format!("account serialize: {e}")))?;
    kc_store(&account_info_key_for(&account_id), &account_json)?;
    // Keep the legacy single-account slot in sync too — older builds on the
    // same machine still read those keys, and silent reauth's migration path
    // uses them when the registry is empty.
    kc_store(KEYRING_REFRESH_ACCOUNT, &refresh_token)?;
    kc_store(KEYRING_ACCOUNT_INFO, &account_json)?;

    // Register the account so the UI can enumerate it.
    if let Err(err) = azure_accounts::upsert(AccountRegistryEntry {
        account_id: account_id.clone(),
        username: account.username.clone(),
        tenant_id: account.tenant_id.clone(),
        display_name: account.name.clone(),
        last_signed_in: Utc::now(),
    }) {
        tracing::warn!(
            target: "queryben::azure::oauth",
            %err,
            "account registry upsert failed after sign-in"
        );
    }

    // Wipe the in-memory bearer cache so tokens minted for the previous
    // account (if any) don't leak into subsequent ARM calls that expect this
    // new account's identity.
    cache.clear_all()?;
    cache.set_account(Some(account.clone()))?;
    let expires_at = Utc::now() + chrono::Duration::seconds(token.expires_in);
    cache.put(
        "https://management.azure.com/.default",
        token.access_token.clone(),
        expires_at,
    )?;

    // Belt-and-suspenders: also write the refresh token + fresh access token to
    // the on-disk file cache. This is the layer that survives keychain wipes
    // (see infra::token_file_cache for the "why"). Keychain write is kept for
    // backwards compat with users on older builds; we'll drop it in a later PR.
    write_through_to_file_cache(&refresh_token, &account, &token.access_token, expires_at);

    Ok(account)
}

/// Mirror the freshly-signed-in state into `token_file_cache`. Failures are
/// logged and swallowed — the keychain-based path still works, so a file-cache
/// write hiccup shouldn't block sign-in.
fn write_through_to_file_cache(
    refresh_token: &str,
    account: &AzureAccount,
    mgmt_access_token: &str,
    mgmt_expires_at: DateTime<Utc>,
) {
    let mut existing = token_file_cache::load().unwrap_or_default();
    existing.refresh_token = Some(refresh_token.to_string());
    existing.account = Some(PersistedAccount {
        tenant_id: account.tenant_id.clone(),
        home_account_id: account.home_account_id.clone(),
        username: account.username.clone(),
    });
    existing.put_access_token(
        "https://management.azure.com/",
        mgmt_access_token.to_string(),
        mgmt_expires_at.timestamp(),
    );
    if let Err(err) = token_file_cache::save(&existing) {
        tracing::warn!(
            target: "queryben::azure::oauth",
            %err,
            "file cache save failed after sign-in (keychain path still active)"
        );
    }
}

/// Silent access-token fetch. Checks the in-memory cache first, otherwise
/// refreshes with the stored refresh token. Never opens a browser.
///
/// `account_id`: when `Some`, resolves the per-account refresh token first;
/// when `None`, falls back to the legacy single-account slot (migration path
/// for connections that predate multi-account).
pub async fn acquire_token(
    cache: &TokenCache,
    tenant_id: &str,
    client_id: &str,
    scope: &str,
    account_id: Option<&str>,
) -> Result<String, AppError> {
    try_acquire_silent(cache, tenant_id, client_id, scope, account_id).await
}

/// Silent-only variant. Returns `Ok(token)` if we could satisfy the request
/// from cache or by trading the stored refresh_token; returns `AuthFailed`
/// **without** opening a browser when interactive sign-in would be needed
/// (no refresh_token in keychain, or the refresh_token was rejected).
/// Network / 5xx errors bubble as their normal `AppError` variants.
///
/// Used by `can_add_rule_silently` to probe whether the auto-firewall path can
/// run without prompting the user. Also the sole path used by `acquire_token`
/// today — we never had an interactive fallback baked into the token
/// acquisition itself; interactive sign-in is a separate `sign_in()` call.
pub async fn try_acquire_silent(
    cache: &TokenCache,
    tenant_id: &str,
    client_id: &str,
    scope: &str,
    account_id: Option<&str>,
) -> Result<String, AppError> {
    if let Some(cached) = cache.get(scope)? {
        return Ok(cached);
    }

    // Resolve which account's refresh token to trade. When the caller supplied
    // an explicit `account_id`, honor it. Otherwise fall back to the only
    // registered account (migration path for legacy connections that were
    // saved before per-account tokens existed) — this preserves the pre-
    // multi-account behavior where every silent reauth used "the one signed-in
    // account" without asking.
    let resolved_account_id: Option<String> = account_id
        .map(str::to_string)
        .or_else(|| azure_accounts::only_account().map(|e| e.account_id));

    // 1. CLI-first: piggyback on `az account get-access-token` when the user
    //    has already `az login`-ed. Matches Azure Data Studio's behavior —
    //    anyone with the CLI signed in never sees our browser sign-in dialog,
    //    even after a fresh install or keychain wipe. Silently returns None
    //    if `az` isn't installed or isn't signed in, so we fall through.
    //
    //    The `QUERYBEN_DISABLE_AZ_CLI` escape hatch exists so integration tests
    //    can deterministically exercise the file-cache branch even on a dev box
    //    where the tester has `az login` already active.
    let az_cli_disabled = std::env::var("QUERYBEN_DISABLE_AZ_CLI")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    if !az_cli_disabled {
    if let Some(cli_token) = crate::adapters::azure_cli::get_access_token_full(scope).await {
        // Seed the in-memory cache so repeat calls within a session skip the
        // spawn. Fall back to a conservative 55-minute TTL when the CLI
        // didn't emit `expires_on` (Azure CLI < 2.54).
        let expires_at = cli_token
            .expires_on
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
            .unwrap_or_else(|| Utc::now() + chrono::Duration::minutes(55));
        cache.put(scope, cli_token.access_token.clone(), expires_at)?;
        return Ok(cli_token.access_token);
    }
    }

    // 1b. Azure Data Studio bridge — if the user is signed into ADS, borrow
    //     their MSAL cache. Same trick az CLI does, but sourced from a
    //     different tool. Silent on macOS after the one-time "Always Allow"
    //     keychain prompt because we cache the borrowed token in the file
    //     cache below so subsequent probes skip the keychain entirely.
    //
    //     `QUERYBEN_DISABLE_BRIDGES` lets integration tests deterministically
    //     exercise the file-cache branch even on a dev box where ADS + VS Code
    //     are both signed in.
    let bridges_disabled = std::env::var("QUERYBEN_DISABLE_BRIDGES")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    if !bridges_disabled {
        if let Some(borrowed) = crate::adapters::ads_bridge::try_borrow_ads_token(scope).await {
            cache_borrowed_token(cache, scope, &borrowed.access_token, borrowed.expires_at)?;
            return Ok(borrowed.access_token);
        }

        // 1c. VS Code Microsoft Authentication bridge. Same story — if the
        //     user's signed into VS Code's Azure Account extension we can
        //     trade their refresh token for a fresh access token.
        if let Some(borrowed) =
            crate::adapters::vscode_bridge::try_borrow_vscode_token(scope).await
        {
            cache_borrowed_token(cache, scope, &borrowed.access_token, borrowed.expires_at)?;
            return Ok(borrowed.access_token);
        }
    }

    // 2. File cache — the ADS-parity durability layer. Loads whatever we last
    //    persisted under <app-data>/QueryBen/azure-cache.json. If it has a
    //    still-valid access token for `scope`, return it directly (no network).
    //    Otherwise if it has a refresh token, trade it, and update the file.
    //
    //    Migration: on first read, if the file cache is empty but the keychain
    //    has a refresh token, copy it over so subsequent launches are file-
    //    cache-native even for users who upgraded through the ADS-parity PR.
    let mut file_cache = token_file_cache::load().unwrap_or_default();
    if file_cache.refresh_token.is_none() {
        // Prefer the per-account keychain slot when we know which account to
        // trade for; fall back to the legacy slot for pre-multi-account tokens.
        let kc_refresh_maybe = if let Some(aid) = resolved_account_id.as_deref() {
            match kc_load(&refresh_key_for(aid))? {
                Some(t) => Some(t),
                None => kc_load(KEYRING_REFRESH_ACCOUNT)?,
            }
        } else {
            kc_load(KEYRING_REFRESH_ACCOUNT)?
        };
        if let Some(kc_refresh) = kc_refresh_maybe {
            file_cache.refresh_token = Some(kc_refresh);
            if let Some(existing_account) = current_account_from_keychain()? {
                file_cache.account = Some(PersistedAccount {
                    tenant_id: existing_account.tenant_id,
                    home_account_id: existing_account.home_account_id,
                    username: existing_account.username,
                });
            }
            if let Err(err) = token_file_cache::save(&file_cache) {
                tracing::warn!(
                    target: "queryben::azure::oauth",
                    %err,
                    "migration: keychain -> file cache save failed"
                );
            } else {
                tracing::info!(
                    target: "queryben::azure::oauth",
                    "migrated keychain refresh token to file cache"
                );
            }
        }
    }

    if let Some(entry) = file_cache.get_valid_access_token(scope) {
        // Non-expired access token in the file cache — hand it back and warm
        // the in-memory cache. No network, no CLI, no keychain touch.
        let expires_at = DateTime::<Utc>::from_timestamp(entry.expires_at_unix, 0)
            .unwrap_or_else(|| Utc::now() + chrono::Duration::minutes(5));
        let token = entry.token.clone();
        cache.put(scope, token.clone(), expires_at)?;
        return Ok(token);
    }

    if let Some(file_refresh) = file_cache.refresh_token.clone() {
        // File has a refresh token but no fresh access token for this scope.
        // Trade it against the token endpoint; on success, update the file so
        // the rotated refresh token isn't lost.
        let refresh_scope = format!("{scope} offline_access");
        let form = [
            ("client_id", client_id),
            ("scope", refresh_scope.as_str()),
            ("refresh_token", file_refresh.as_str()),
            ("grant_type", "refresh_token"),
        ];
        match post_token_form(tenant_id, &form).await {
            Ok(token) => {
                if let Some(new_refresh) = token.refresh_token.as_deref() {
                    file_cache.refresh_token = Some(new_refresh.to_string());
                    // Rotate both the per-account slot (if known) and the
                    // legacy single-account slot so older builds still work.
                    if let Some(aid) = resolved_account_id.as_deref() {
                        let _ = kc_store(&refresh_key_for(aid), new_refresh);
                    }
                    let _ = kc_store(KEYRING_REFRESH_ACCOUNT, new_refresh);
                }
                let expires_at = Utc::now() + chrono::Duration::seconds(token.expires_in);
                file_cache.put_access_token(
                    scope,
                    token.access_token.clone(),
                    expires_at.timestamp(),
                );
                if let Err(err) = token_file_cache::save(&file_cache) {
                    tracing::warn!(
                        target: "queryben::azure::oauth",
                        %err,
                        "file cache save failed after refresh (in-memory still ok)"
                    );
                }
                cache.put(scope, token.access_token.clone(), expires_at)?;
                return Ok(token.access_token);
            }
            Err(AppError::AuthFailed(msg)) => {
                // Refresh token in the file cache is dead. Fall through to the
                // keychain path — the keychain copy might have been rotated
                // more recently (rare but possible if two builds ran side by
                // side). If both are dead the keychain branch will surface the
                // "sign in again" error.
                tracing::debug!(
                    target: "queryben::azure::oauth",
                    %msg,
                    "file cache refresh token rejected, falling through to keychain"
                );
            }
            Err(other) => return Err(other),
        }
    }

    // 3. Keychain fallback — the pre-file-cache path. Kept in for backwards
    //    compat and for the sign-in-flow-hasn't-run-yet-on-this-launch case.
    //    Try the per-account slot first, then the legacy single-account slot.
    let refresh_token = match resolved_account_id.as_deref() {
        Some(aid) => kc_load(&refresh_key_for(aid))?
            .or(kc_load(KEYRING_REFRESH_ACCOUNT)?)
            .ok_or_else(|| AppError::AuthFailed("not signed in; call azure_sign_in first".into()))?,
        None => kc_load(KEYRING_REFRESH_ACCOUNT)?
            .ok_or_else(|| AppError::AuthFailed("not signed in; call azure_sign_in first".into()))?,
    };

    // Ask only for the resource we need. Azure returns an access token whose
    // `aud` matches it, plus a rotated refresh token to persist.
    let refresh_scope = format!("{scope} offline_access");
    let form = [
        ("client_id", client_id),
        ("scope", refresh_scope.as_str()),
        ("refresh_token", refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];

    let token = match post_token_form(tenant_id, &form).await {
        Ok(t) => t,
        Err(AppError::AuthFailed(msg)) => {
            // Refresh token dead (revoked, expired, password changed). Wipe
            // the slots we tried so callers surface "sign in again" instead of
            // looping. Only clear the file cache when we didn't have a specific
            // account_id — otherwise we'd blow away other accounts' state.
            if let Some(aid) = resolved_account_id.as_deref() {
                let _ = kc_delete(&refresh_key_for(aid));
            } else {
                let _ = kc_delete(KEYRING_REFRESH_ACCOUNT);
                token_file_cache::clear();
                cache.clear_all()?;
            }
            return Err(AppError::AuthFailed(format!(
                "refresh token rejected, sign in again ({msg})"
            )));
        }
        Err(other) => return Err(other),
    };

    // Rotate the stored refresh token if Azure sent a new one, in every slot
    // we may have read from.
    if let Some(new_refresh) = token.refresh_token.as_deref() {
        if let Some(aid) = resolved_account_id.as_deref() {
            kc_store(&refresh_key_for(aid), new_refresh)?;
        }
        kc_store(KEYRING_REFRESH_ACCOUNT, new_refresh)?;
        file_cache.refresh_token = Some(new_refresh.to_string());
    }

    let expires_at = Utc::now() + chrono::Duration::seconds(token.expires_in);
    file_cache.put_access_token(scope, token.access_token.clone(), expires_at.timestamp());
    if let Err(err) = token_file_cache::save(&file_cache) {
        tracing::debug!(
            target: "queryben::azure::oauth",
            %err,
            "file cache save failed after keychain-tier refresh"
        );
    }
    cache.put(scope, token.access_token.clone(), expires_at)?;

    Ok(token.access_token)
}

/// Persist a token borrowed from ADS or VS Code into the in-memory + file
/// caches so subsequent calls in this session (and in later launches) can be
/// served without re-reading the source-tool keychain items. This is what
/// gives us the "one 'Always Allow' prompt, then silent forever" behavior —
/// the next probe hits the file cache first and never touches the keychain.
fn cache_borrowed_token(
    cache: &TokenCache,
    scope: &str,
    access_token: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    cache.put(scope, access_token.to_string(), expires_at)?;
    let mut file_cache = token_file_cache::load().unwrap_or_default();
    file_cache.put_access_token(scope, access_token.to_string(), expires_at.timestamp());
    if let Err(err) = token_file_cache::save(&file_cache) {
        tracing::debug!(
            target: "queryben::azure::oauth",
            %err,
            "file cache save failed after ADS/VS Code borrow"
        );
    }
    Ok(())
}

/// Read-only helper: pull the persisted `AzureAccount` out of the keychain
/// without any network. Used by the migration path in `try_acquire_silent` so
/// we can hydrate the file cache's `account` field with what the keychain has.
fn current_account_from_keychain() -> Result<Option<AzureAccount>, AppError> {
    let Some(json) = kc_load(KEYRING_ACCOUNT_INFO)? else {
        return Ok(None);
    };
    let account: AzureAccount = serde_json::from_str(&json)
        .map_err(|e| AppError::internal(format!("account info decode: {e}")))?;
    Ok(Some(account))
}

/// Read persisted account, no prompt. Populates the in-memory mirror on first
/// call after launch. Checks in order: in-memory → registry (most-recent
/// entry) → legacy keychain → file cache. The registry lookup is what surfaces
/// the "currently active" account for multi-account setups.
pub fn current_account(cache: &TokenCache) -> Result<Option<AzureAccount>, AppError> {
    if let Some(a) = cache.get_account()? {
        return Ok(Some(a));
    }
    let registry = azure_accounts::load();
    if !registry.is_empty() {
        // Most recently signed-in wins as the "current" account.
        let mut sorted = registry;
        sorted.sort_by(|a, b| b.last_signed_in.cmp(&a.last_signed_in));
        let latest = &sorted[0];
        // Prefer the richer per-account info blob if present (has the `name`
        // claim from the id token). Fall back to synthesizing from the
        // registry entry for accounts that pre-date the info-blob write.
        let account = match kc_load(&account_info_key_for(&latest.account_id))? {
            Some(json) => serde_json::from_str::<AzureAccount>(&json)
                .map_err(|e| AppError::internal(format!("account info decode: {e}")))?,
            None => AzureAccount {
                id: latest.account_id.split('.').next().unwrap_or("").to_string(),
                username: latest.username.clone(),
                tenant_id: latest.tenant_id.clone(),
                home_account_id: latest.account_id.clone(),
                name: latest.display_name.clone(),
            },
        };
        cache.set_account(Some(account.clone()))?;
        return Ok(Some(account));
    }
    if let Some(json) = kc_load(KEYRING_ACCOUNT_INFO)? {
        // Account info but no refresh token means the user cleared the
        // keychain by hand. Fall through to the file cache instead of
        // returning None — the file cache is exactly what we added to
        // survive that case.
        if kc_load(KEYRING_REFRESH_ACCOUNT)?.is_some() {
            let account: AzureAccount = serde_json::from_str(&json)
                .map_err(|e| AppError::internal(format!("account info decode: {e}")))?;
            cache.set_account(Some(account.clone()))?;
            return Ok(Some(account));
        }
    }

    // File-cache fallback. If the on-disk file has both an account record and
    // a refresh token, the user is effectively still signed in even though the
    // keychain is empty (dev churn, reinstall, manual clear).
    let file = token_file_cache::load().unwrap_or_default();
    let (Some(persisted), Some(_refresh)) = (file.account.as_ref(), file.refresh_token.as_ref())
    else {
        return Ok(None);
    };
    let account = AzureAccount {
        id: persisted.home_account_id.split('.').next().unwrap_or("").to_string(),
        username: persisted.username.clone(),
        tenant_id: persisted.tenant_id.clone(),
        home_account_id: persisted.home_account_id.clone(),
        name: None,
    };
    cache.set_account(Some(account.clone()))?;
    Ok(Some(account))
}

/// Wipe every account's keychain slot + file cache + in-memory cache + the
/// account registry. Idempotent.
pub fn sign_out(cache: &TokenCache) -> Result<(), AppError> {
    // Per-account keychain slots first — one delete per registered account.
    for entry in azure_accounts::load() {
        let _ = kc_delete(&refresh_key_for(&entry.account_id));
        let _ = kc_delete(&account_info_key_for(&entry.account_id));
    }
    kc_delete(KEYRING_REFRESH_ACCOUNT)?;
    kc_delete(KEYRING_ACCOUNT_INFO)?;
    let _ = azure_accounts::save(&[]);
    token_file_cache::clear();
    cache.clear_all()?;
    Ok(())
}

/// Sign out a single account. Deletes just that account's keychain slots +
/// registry entry. Callers must separately mark any dependent connections as
/// "needs reconnect" — this function never touches connection data.
pub fn sign_out_account(cache: &TokenCache, account_id: &str) -> Result<(), AppError> {
    let _ = kc_delete(&refresh_key_for(account_id));
    let _ = kc_delete(&account_info_key_for(account_id));

    let remaining = azure_accounts::remove(account_id)?;

    // If nothing is left, wipe the legacy slots + file cache too — otherwise a
    // stale refresh token could still let silent reauth mint tokens for an
    // "account" the user believes is signed out.
    if remaining.is_empty() {
        kc_delete(KEYRING_REFRESH_ACCOUNT)?;
        kc_delete(KEYRING_ACCOUNT_INFO)?;
        token_file_cache::clear();
        cache.clear_all()?;
    } else {
        // Just drop the in-memory bearer cache — the file cache might still
        // hold access tokens minted from this account's refresh token, but
        // those expire within the hour and re-mint won't succeed without the
        // now-deleted keychain entry, so treating them as poisoned is safe.
        cache.clear_all()?;
    }

    Ok(())
}

/// List every signed-in account. Read-only.
pub fn list_accounts() -> Vec<AccountRegistryEntry> {
    azure_accounts::load()
}

/// Backfill the account registry from whatever the legacy single-account
/// keychain slot has. Returns the resolved `account_id` when the promotion
/// succeeded, `None` when there was nothing to promote or the id token
/// couldn't be recovered. Callers use the return value to backfill
/// `Connection.account_id` on legacy rows.
///
/// Never overwrites an existing registry entry. Safe to call on every launch.
pub fn migrate_legacy_account_if_needed() -> Result<Option<String>, AppError> {
    if !azure_accounts::load().is_empty() {
        return Ok(None);
    }

    let Some(json) = kc_load(KEYRING_ACCOUNT_INFO)? else {
        // No legacy account_info blob → nothing to promote. Silent reauth will
        // still work through the legacy KEYRING_REFRESH_ACCOUNT slot, but we
        // have no way to name the account, so connections stay with
        // account_id = None.
        return Ok(None);
    };
    let account: AzureAccount = match serde_json::from_str(&json) {
        Ok(a) => a,
        Err(err) => {
            tracing::info!(
                target: "queryben::azure::oauth",
                %err,
                "legacy account_info couldn't be decoded; leaving registry empty"
            );
            return Ok(None);
        }
    };

    let account_id = account.home_account_id.clone();
    azure_accounts::upsert(AccountRegistryEntry {
        account_id: account_id.clone(),
        username: account.username.clone(),
        tenant_id: account.tenant_id.clone(),
        display_name: account.name.clone(),
        last_signed_in: Utc::now(),
    })?;

    // Also mirror the legacy refresh + account_info into the per-account slots
    // so subsequent silent reauths hit the account-scoped keys.
    if let Some(refresh) = kc_load(KEYRING_REFRESH_ACCOUNT)? {
        kc_store(&refresh_key_for(&account_id), &refresh)?;
    }
    kc_store(&account_info_key_for(&account_id), &json)?;

    tracing::info!(
        target: "queryben::azure::oauth",
        %account_id,
        username = %account.username,
        "migrated legacy single-account tokens into per-account registry"
    );

    Ok(Some(account_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_key_scopes_by_account_id() {
        let a = refresh_key_for("oid1.tid1");
        let b = refresh_key_for("oid2.tid1");
        assert_ne!(a, b, "distinct account ids must yield distinct keychain keys");
        assert!(a.contains("oid1.tid1"));
        assert!(b.contains("oid2.tid1"));
    }

    #[test]
    fn refresh_key_does_not_collide_with_legacy_slot() {
        let per_account = refresh_key_for("any-id");
        assert_ne!(per_account, KEYRING_REFRESH_ACCOUNT);
    }

    #[test]
    fn account_info_key_scopes_by_account_id() {
        let a = account_info_key_for("oid1.tid1");
        let b = account_info_key_for("oid2.tid1");
        assert_ne!(a, b);
        assert!(a.contains("oid1.tid1"));
        assert!(b.contains("oid2.tid1"));
    }

    #[test]
    fn account_info_key_does_not_collide_with_refresh_key() {
        let aid = "oid1.tid1";
        assert_ne!(refresh_key_for(aid), account_info_key_for(aid));
    }

    #[test]
    fn parse_id_token_extracts_home_account_id() {
        // Minimal id_token: header.payload.signature, base64url-nopad.
        let claims = serde_json::json!({
            "oid": "user-oid-123",
            "tid": "tenant-abc",
            "preferred_username": "alice@example.com",
            "name": "Alice",
        });
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap_or_default());
        let jwt = format!("HEADER.{payload}.SIG");
        let acc = parse_id_token(&jwt).expect("parse ok");
        assert_eq!(acc.id, "user-oid-123");
        assert_eq!(acc.tenant_id, "tenant-abc");
        assert_eq!(acc.home_account_id, "user-oid-123.tenant-abc");
        assert_eq!(acc.username, "alice@example.com");
    }
}
