//! AES-256-CBC decryption of the ADS accessTokenCache blob.
//!
//! The key + IV round-trip through the OS credential store as UTF-8 strings —
//! ADS wrote them via `Buffer.toString('utf16le')`, so recovering the raw
//! bytes means re-encoding the string as UTF-16LE.

use aes::Aes256;
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub(super) fn decrypt_cache_file(
    path: &std::path::Path,
    key: &[u8],
    iv: &[u8],
) -> Result<String, DecryptError> {
    let raw = std::fs::read(path).map_err(|_| DecryptError::Io)?;
    let ct = BASE64_STANDARD.decode(&raw).map_err(|_| DecryptError::Base64)?;

    if key.len() != 32 || iv.len() != 16 {
        return Err(DecryptError::KeySize);
    }

    let mut buf = ct;
    let plaintext = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|_| DecryptError::Cipher)?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| DecryptError::Unpad)?;

    String::from_utf8(plaintext.to_vec()).map_err(|_| DecryptError::Utf8)
}

#[derive(Debug)]
pub(super) enum DecryptError {
    Io,
    Base64,
    KeySize,
    Cipher,
    Unpad,
    Utf8,
}

impl std::fmt::Display for DecryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io => "read failed",
            Self::Base64 => "base64 decode failed",
            Self::KeySize => "key/iv wrong size",
            Self::Cipher => "cipher init failed",
            Self::Unpad => "PKCS7 unpad failed (wrong key?)",
            Self::Utf8 => "plaintext not UTF-8",
        })
    }
}

/// Convert a UTF-8 string (as the credential store returns it) into the raw
/// byte sequence ADS originally serialized via `Buffer.toString('utf16le')`.
pub(super) fn utf8_string_to_utf16le_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.encode_utf16().count() * 2);
    for u in s.encode_utf16() {
        out.push((u & 0xff) as u8);
        out.push((u >> 8) as u8);
    }
    out
}

pub(super) fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(hex_decode("00ff10").as_deref(), Some(&[0x00, 0xff, 0x10][..]));
        assert!(hex_decode("0f0").is_none());
        assert!(hex_decode("zz").is_none());
    }

    #[test]
    fn utf16le_roundtrip() {
        let raw: Vec<u8> = (0u8..32).collect();
        let s: String = std::char::decode_utf16(
            raw.chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]])),
        )
        .filter_map(Result::ok)
        .collect();
        let back = utf8_string_to_utf16le_bytes(&s);
        assert_eq!(back, raw);
    }
}
