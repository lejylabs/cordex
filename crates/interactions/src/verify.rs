//! Ed25519 verification of HTTP interactions.
//!
//! Discord signs `timestamp + body` with the application's key and sends
//! `X-Signature-Ed25519` (hex) and `X-Signature-Timestamp`. Verify on the
//! raw body **before** parsing JSON; answer `401` when it fails.
//!
//! Works in any web framework:
//!
//! ```
//! use cordex_interactions::verify::{PublicKey, verify_signature};
//!
//! // "Public Key" from the developer portal (General Information).
//! let key = PublicKey::from_hex(
//!     "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c",
//! ).unwrap();
//! let ok = verify_signature(&key, "00".repeat(64).as_str(), "1700000000", b"{}");
//! assert!(ok.is_err());
//! ```

use ed25519_dalek::{Signature, VerifyingKey};

use crate::error::InteractionError;

/// Header with the hex signature.
pub const SIGNATURE_HEADER: &str = "x-signature-ed25519";
/// Header with the signed timestamp.
pub const TIMESTAMP_HEADER: &str = "x-signature-timestamp";

/// The application's public key (developer portal → General Information).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// Parse the 64-character hex key shown in the developer portal.
    pub fn from_hex(hex: &str) -> Result<Self, InteractionError> {
        let bytes: [u8; 32] =
            decode_hex(hex.trim()).ok_or_else(|| invalid_key("expected 64 hex characters"))?;
        Self::from_bytes(&bytes)
    }

    /// Key from its 32 raw bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, InteractionError> {
        VerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| invalid_key("not a valid Ed25519 point"))
    }
}

/// Check `signature_hex` over `timestamp + body`. Uses strict verification
/// (rejects malleable and small-order signatures).
pub fn verify_signature(
    key: &PublicKey,
    signature_hex: &str,
    timestamp: &str,
    body: &[u8],
) -> Result<(), InteractionError> {
    let sig: [u8; 64] = decode_hex(signature_hex.trim()).ok_or(InteractionError::BadSignature)?;
    let signature = Signature::from_bytes(&sig);
    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);
    key.0
        .verify_strict(&message, &signature)
        .map_err(|_| InteractionError::BadSignature)
}

fn invalid_key(why: &str) -> InteractionError {
    InteractionError::InvalidPublicKey(format!("invalid public key: {why}").into())
}

/// Decode exactly `N` bytes of hex (upper- or lowercase).
fn decode_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    let raw = hex.as_bytes();
    if raw.len() != N * 2 {
        return None;
    }
    let mut out = [0_u8; N];
    for (byte, pair) in out.iter_mut().zip(raw.as_chunks::<2>().0) {
        *byte = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(out)
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    pub(crate) fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    pub(crate) fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    pub(crate) fn sign(timestamp: &str, body: &[u8]) -> String {
        let mut msg = timestamp.as_bytes().to_vec();
        msg.extend_from_slice(body);
        hex(&signing_key().sign(&msg).to_bytes())
    }

    pub(crate) fn public_key() -> PublicKey {
        PublicKey::from_hex(&hex(signing_key().verifying_key().as_bytes())).unwrap()
    }

    #[test]
    fn valid_and_tampered() {
        let key = public_key();
        let sig = sign("1700000000", b"{\"type\":1}");
        verify_signature(&key, &sig, "1700000000", b"{\"type\":1}").unwrap();
        verify_signature(&key, &sig.to_uppercase(), "1700000000", b"{\"type\":1}").unwrap();
        assert!(verify_signature(&key, &sig, "1700000001", b"{\"type\":1}").is_err());
        assert!(verify_signature(&key, &sig, "1700000000", b"{\"type\":2}").is_err());
        assert!(verify_signature(&key, "zz", "1700000000", b"{}").is_err());
        assert!(verify_signature(&key, &"0".repeat(128), "1700000000", b"{}").is_err());
    }

    #[test]
    fn key_parsing() {
        assert!(PublicKey::from_hex("abc").is_err());
        assert!(PublicKey::from_hex(&"g".repeat(64)).is_err());
        assert_eq!(public_key(), public_key());
    }
}
