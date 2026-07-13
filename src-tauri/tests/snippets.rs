//! Integration tests for the user-snippets file loader. Mirrors the
//! theme-override pattern: missing file → Ok(None); valid JSON → Ok(Some);
//! malformed JSON → Ok(None) (never surfaces as an error).

use std::fs;
use tempfile::TempDir;

use queryben_lib::ipc::snippets::{load_snippets_from, SNIPPETS_FILE};

#[test]
fn returns_none_when_file_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let out = load_snippets_from(tmp.path()).expect("load");
    assert!(out.is_none(), "expected None for missing file, got {out:?}");
}

#[test]
fn returns_some_raw_text_when_valid() {
    let tmp = TempDir::new().expect("tempdir");
    let payload = r#"[
      {"id":"user.demo","name":"demo","description":"","language":"sql","body":"SELECT 1","tags":["demo"]}
    ]"#;
    fs::write(tmp.path().join(SNIPPETS_FILE), payload).expect("write");

    let out = load_snippets_from(tmp.path()).expect("load");
    let raw = out.expect("some");
    assert!(raw.contains("user.demo"));
    // Frontend parses — we just hand back bytes verbatim.
    assert_eq!(raw, payload);
}

#[test]
fn returns_none_for_malformed_json() {
    let tmp = TempDir::new().expect("tempdir");
    fs::write(tmp.path().join(SNIPPETS_FILE), b"{not valid json,,,").expect("write");

    let out = load_snippets_from(tmp.path()).expect("load");
    assert!(out.is_none(), "malformed JSON should surface as None");
}

#[test]
fn returns_none_for_non_utf8() {
    let tmp = TempDir::new().expect("tempdir");
    let bad: [u8; 4] = [0xFF, 0xFE, 0xFD, 0xFC];
    fs::write(tmp.path().join(SNIPPETS_FILE), bad).expect("write");

    let out = load_snippets_from(tmp.path()).expect("load");
    assert!(out.is_none(), "non-UTF8 payload should surface as None");
}

#[test]
fn accepts_empty_json_array() {
    let tmp = TempDir::new().expect("tempdir");
    fs::write(tmp.path().join(SNIPPETS_FILE), b"[]").expect("write");

    let out = load_snippets_from(tmp.path()).expect("load");
    assert_eq!(out.as_deref(), Some("[]"));
}
