//! Live probe against the real ADS cache on this dev machine.
//!
//! Marked `#[ignore]` so `cargo test` never runs it in CI or in the default
//! developer loop — only `scripts/verify-ads-bridge.sh` invokes it, which
//! passes `--ignored`. Uses the actual OS credential store.
//!
//! Print output is grepped by the wrapping shell script — keep the OK / FAIL
//! / SKIP prefixes stable.

use queryben_lib::adapters::ads_bridge;

/// Fully self-diagnosing probe. Prints one of:
///
///   * `OK: borrowed token ...`    — every stage worked, real token in hand.
///   * `PARTIAL: decrypt OK, but ADS account can't mint <scope> tokens (...)`
///       — the ADS cache decrypted cleanly (proving the bridge itself works),
///       but the specific refresh token can't reach the requested resource.
///       This is the expected outcome when the user has ONLY a Microsoft
///       personal (MSA / consumer) account signed into ADS: MSA accounts
///       can't call ARM regardless of which client tries. To turn this into
///       an OK the user just needs to also sign into an AAD org account in ADS.
///   * `FAIL: <mode>` — a specific failure mode (see script comments).
///   * `SKIP: no ADS cache at ...` — ADS never installed / never signed in.
#[test]
#[ignore]
fn live_probe() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("queryben=info")),
        )
        .try_init();

    // Step 1: prove the file / keychain / decrypt half of the pipeline works.
    // If this fails, no scope trade is possible.
    let decrypt_ok = check_decrypt_pipeline();
    if !decrypt_ok {
        println!("FAIL: could not decrypt ADS cache (see logs above)");
        return;
    }

    // Step 2: try the network trade against the resources most users care
    // about. Success on any = OK. All 4xx with a diagnosable reason = PARTIAL.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let scopes = [
        "https://database.windows.net/",
        "https://management.azure.com/",
        "https://graph.microsoft.com/",
    ];
    let mut winning_scope = None;
    for scope in scopes {
        if let Some(t) = rt.block_on(ads_bridge::try_borrow_ads_token(scope)) {
            println!(
                "OK: borrowed {} token ({} bytes) expires {}",
                scope,
                t.access_token.len(),
                t.expires_at.to_rfc3339()
            );
            winning_scope = Some(scope);
            break;
        }
    }
    if winning_scope.is_none() {
        // Decrypt worked (asserted above) so this is a legit account/scope
        // mismatch — most commonly the "ADS is signed in with a personal
        // Microsoft account only" case documented at the top of this file.
        println!(
            "PARTIAL: decrypt OK, but ADS account can't mint tokens for any of {:?} \
             (likely an MSA/personal-account-only sign-in — sign into ADS with an \
             AAD org account to enable the bridge for ARM / SQL scopes)",
            scopes
        );
    }
}

/// Read key/iv from the keychain, decrypt the cache file, sanity-check the
/// resulting JSON. Returns true iff we got a well-formed MSAL cache back.
/// Prints one diagnostic line either way.
#[cfg(target_os = "macos")]
fn check_decrypt_pipeline() -> bool {
    use aes::Aes256;
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    use base64::Engine as _;
    use queryben_lib::adapters::keychain;
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    let key_str = match keychain::get_password(
        "azureAccountProviderCredentials|accessTokenCache-key",
        "",
    ) {
        Ok(Some(v)) => v,
        Ok(None) => {
            println!("FAIL: keychain has no accessTokenCache-key (ADS not signed in?)");
            return false;
        }
        Err(e) => {
            println!("FAIL: keychain read errored ({:?})", e);
            return false;
        }
    };
    let iv_str = match keychain::get_password(
        "azureAccountProviderCredentials|accessTokenCache-iv",
        "",
    ) {
        Ok(Some(v)) => v,
        Ok(None) => {
            println!("FAIL: keychain has no accessTokenCache-iv");
            return false;
        }
        Err(e) => {
            println!("FAIL: keychain iv read errored ({:?})", e);
            return false;
        }
    };
    let to_bytes = |s: &str| -> Vec<u8> {
        let mut out = Vec::new();
        for u in s.encode_utf16() {
            out.push((u & 0xff) as u8);
            out.push((u >> 8) as u8);
        }
        out
    };
    let key = to_bytes(&key_str);
    let iv = to_bytes(&iv_str);
    if key.len() != 32 || iv.len() != 16 {
        println!("FAIL: key/iv wrong size (got {}/{})", key.len(), iv.len());
        return false;
    }
    let cache = std::path::PathBuf::from(std::env::var("HOME").expect("HOME"))
        .join("Library/Application Support/azuredatastudio/Azure Accounts/accessTokenCache");
    if !cache.exists() {
        println!("SKIP: no ADS cache at {:?}", cache);
        return false;
    }
    let raw = match std::fs::read(&cache) {
        Ok(v) => v,
        Err(e) => {
            println!("FAIL: read cache ({e})");
            return false;
        }
    };
    let mut ct = match base64::engine::general_purpose::STANDARD.decode(&raw) {
        Ok(v) => v,
        Err(e) => {
            println!("FAIL: base64 decode ({e})");
            return false;
        }
    };
    let pt = match Aes256CbcDec::new_from_slices(&key, &iv) {
        Ok(c) => match c.decrypt_padded_mut::<Pkcs7>(&mut ct) {
            Ok(p) => p,
            Err(e) => {
                println!("FAIL: decrypt/unpad ({e}) — key rotated?");
                return false;
            }
        },
        Err(e) => {
            println!("FAIL: cipher init ({e})");
            return false;
        }
    };
    let s = match std::str::from_utf8(pt) {
        Ok(v) => v,
        Err(e) => {
            println!("FAIL: plaintext not UTF-8 ({e})");
            return false;
        }
    };
    let v: serde_json::Value = match serde_json::from_str(s) {
        Ok(v) => v,
        Err(e) => {
            println!("FAIL: plaintext not JSON ({e})");
            return false;
        }
    };
    let rt_count = v
        .get("RefreshToken")
        .and_then(|x| x.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "[decrypt] OK — plaintext {} bytes, RefreshToken entries: {}",
        s.len(),
        rt_count
    );
    rt_count > 0
}

#[cfg(not(target_os = "macos"))]
fn check_decrypt_pipeline() -> bool {
    println!("SKIP: live decrypt probe only implemented on macOS");
    false
}
