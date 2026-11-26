//! Stateless HMAC-SHA256 signed `custom_id` payloads within Discord's 100-char limit.
//!
//! Lets persistent buttons and menus survive bot restarts without a database
//! while preventing users from forging `owner_id`, `page`, or expired actions.

use std::time::{SystemTime, UNIX_EPOCH};

use cordex_model::id::UserId;
use thiserror::Error;

/// Discord's maximum length for a component `custom_id`.
pub const MAX_CUSTOM_ID_LEN: usize = 100;

/// Truncated HMAC-SHA256 tag length in bytes (64 bits, 11 Base64url chars).
const TAG_LEN: usize = 8;

/// Errors returned when signing or verifying a [`SignedCustomId`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CustomIdError {
    /// Encoded `custom_id` exceeds Discord's 100-character limit.
    #[error("signed custom_id is {len} chars (max {MAX_CUSTOM_ID_LEN})")]
    TooLong {
        /// Actual encoded length.
        len: usize,
    },
    /// Malformed format or invalid Base64url encoding.
    #[error("malformed signed custom_id")]
    Malformed,
    /// HMAC-SHA256 signature verification failed.
    #[error("invalid custom_id signature")]
    InvalidSignature,
    /// Payload has an `expires_at` timestamp in the past.
    #[error("signed custom_id has expired")]
    Expired,
}

/// Compact typed payload encoded inside a signed component `custom_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCustomId {
    /// Short action identifier (e.g. `"page"`, `"ticket_close"`).
    pub action: String,
    /// Optional user ID allowed to interact with this component.
    pub owner_id: Option<UserId>,
    /// Optional numeric page or item index.
    pub page: Option<u32>,
    /// Optional Unix timestamp (seconds) after which the payload is rejected.
    pub expires_at: Option<u64>,
    /// Optional short extra string.
    pub extra: Option<String>,
}

impl SignedCustomId {
    /// Start a new signed payload with `action`.
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            owner_id: None,
            page: None,
            expires_at: None,
            extra: None,
        }
    }

    /// Bind the payload to a specific `owner_id`.
    #[must_use]
    pub const fn owner(mut self, owner_id: UserId) -> Self {
        self.owner_id = Some(owner_id);
        self
    }

    /// Attach a page/index number.
    #[must_use]
    pub const fn page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Set an expiration Unix timestamp in seconds.
    #[must_use]
    pub const fn expires_at(mut self, unix_secs: u64) -> Self {
        self.expires_at = Some(unix_secs);
        self
    }

    /// Attach a short extra string.
    #[must_use]
    pub fn extra(mut self, extra: impl Into<String>) -> Self {
        self.extra = Some(extra.into());
        self
    }

    fn encode_plain(&self) -> String {
        let owner = self
            .owner_id
            .map_or(String::new(), |id| id.get().to_string());
        let page = self.page.map_or(String::new(), |p| p.to_string());
        let exp = self.expires_at.map_or(String::new(), |e| e.to_string());
        let extra = self.extra.as_deref().unwrap_or("");
        format!("{}|{}|{}|{}|{}", self.action, owner, page, exp, extra)
    }

    fn decode_plain(plain: &str) -> Result<Self, CustomIdError> {
        let mut parts = plain.splitn(5, '|');
        let action = parts.next().ok_or(CustomIdError::Malformed)?.to_owned();
        let owner_str = parts.next().ok_or(CustomIdError::Malformed)?;
        let page_str = parts.next().ok_or(CustomIdError::Malformed)?;
        let exp_str = parts.next().ok_or(CustomIdError::Malformed)?;
        let extra_str = parts.next().ok_or(CustomIdError::Malformed)?;

        let owner_id = if owner_str.is_empty() {
            None
        } else {
            Some(
                owner_str
                    .parse::<u64>()
                    .ok()
                    .and_then(UserId::new)
                    .ok_or(CustomIdError::Malformed)?,
            )
        };
        let page = if page_str.is_empty() {
            None
        } else {
            Some(
                page_str
                    .parse::<u32>()
                    .map_err(|_| CustomIdError::Malformed)?,
            )
        };
        let expires_at = if exp_str.is_empty() {
            None
        } else {
            Some(
                exp_str
                    .parse::<u64>()
                    .map_err(|_| CustomIdError::Malformed)?,
            )
        };
        let extra = (!extra_str.is_empty()).then(|| extra_str.to_owned());

        Ok(Self {
            action,
            owner_id,
            page,
            expires_at,
            extra,
        })
    }
}

/// HMAC-SHA256 signer and verifier for [`SignedCustomId`] values.
#[derive(Clone)]
pub struct CustomIdSigner {
    secret: Box<[u8]>,
}

impl CustomIdSigner {
    /// Create a signer from a secret key (e.g. derived from a bot secret env var).
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret: secret.as_ref().into(),
        }
    }

    /// Encode and sign `payload` as a compact Base64url string (`<= 100` chars).
    pub fn sign(&self, payload: &SignedCustomId) -> Result<String, CustomIdError> {
        let plain = payload.encode_plain();
        let mac = hmac_sha256(&self.secret, plain.as_bytes());
        let encoded = format!(
            "{}.{}",
            b64url_encode(plain.as_bytes()),
            b64url_encode(&mac[..TAG_LEN])
        );
        if encoded.len() > MAX_CUSTOM_ID_LEN {
            return Err(CustomIdError::TooLong { len: encoded.len() });
        }
        Ok(encoded)
    }

    /// Verify signature and expiration against `now_unix_secs`, returning the decoded payload.
    pub fn verify(&self, raw: &str, now_unix_secs: u64) -> Result<SignedCustomId, CustomIdError> {
        let (body_b64, tag_b64) = raw.split_once('.').ok_or(CustomIdError::Malformed)?;
        let body = b64url_decode(body_b64).ok_or(CustomIdError::Malformed)?;
        let tag = b64url_decode(tag_b64).ok_or(CustomIdError::Malformed)?;
        if tag.len() != TAG_LEN {
            return Err(CustomIdError::InvalidSignature);
        }
        let expected = hmac_sha256(&self.secret, &body);
        if !ct_eq(&tag, &expected[..TAG_LEN]) {
            return Err(CustomIdError::InvalidSignature);
        }
        let plain = String::from_utf8(body).map_err(|_| CustomIdError::Malformed)?;
        let decoded = SignedCustomId::decode_plain(&plain)?;
        if let Some(exp) = decoded.expires_at
            && now_unix_secs > exp
        {
            return Err(CustomIdError::Expired);
        }
        Ok(decoded)
    }

    /// Verify using the current wall-clock time.
    pub fn verify_now(&self, raw: &str) -> Result<SignedCustomId, CustomIdError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.verify(raw, now)
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// --- Minimal pure-Rust SHA-256 & HMAC-SHA256 (FIPS 180-4 / RFC 2104) ---

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha256(key);
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Vec::with_capacity(64 + data.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);

    let mut outer = [0u8; 96];
    outer[..64].copy_from_slice(&opad);
    outer[64..].copy_from_slice(&inner_hash);
    sha256(&outer)
}

#[allow(clippy::unreadable_literal, clippy::many_single_char_names)]
fn sha256(msg: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (msg.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(msg.len() + 72);
    padded.extend_from_slice(msg);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// --- Base64url (RFC 4648 §5, no padding) ---

const B64_ENC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(B64_ENC[((triple >> 18) & 0x3f) as usize]));
        out.push(char::from(B64_ENC[((triple >> 12) & 0x3f) as usize]));
        if chunk.len() > 1 {
            out.push(char::from(B64_ENC[((triple >> 6) & 0x3f) as usize]));
        }
        if chunk.len() > 2 {
            out.push(char::from(B64_ENC[(triple & 0x3f) as usize]));
        }
    }
    out
}

fn b64url_val(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn b64url_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let v0 = b64url_val(chunk[0])?;
        let v1 = b64url_val(chunk[1])?;
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if let Some(&c2) = chunk.get(2) {
            let v2 = b64url_val(c2)?;
            out.push((((v1 & 0x0f) << 4) | (v2 >> 2)) as u8);
            if let Some(&c3) = chunk.get(3) {
                let v3 = b64url_val(c3)?;
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    #[test]
    fn sign_verify_tamper_and_expiry() {
        let signer = CustomIdSigner::new(b"super-secret-bot-key");
        let owner =
            UserId::new(123_456_789_012_345_678).unwrap_or_else(|| UserId::from(NonZeroU64::MIN));
        let payload = SignedCustomId::new("pg")
            .owner(owner)
            .page(3)
            .expires_at(1_800_000_000);

        let token = signer.sign(&payload);
        assert!(token.is_ok());
        if let Ok(token) = token {
            assert!(token.len() <= MAX_CUSTOM_ID_LEN);
            assert_eq!(signer.verify(&token, 1_799_999_999), Ok(payload));
            assert_eq!(
                signer.verify(&token, 1_800_000_001),
                Err(CustomIdError::Expired)
            );
            let mut tampered = token.into_bytes();
            tampered[0] ^= 0x01;
            let tampered = String::from_utf8_lossy(&tampered).into_owned();
            assert!(signer.verify(&tampered, 1_799_999_999).is_err());
        }
    }
}
