//! CDN image hashes (avatars, icons, banners, …).

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An image hash such as `a_1269e74af4df7417b13759eae50c83dc`.
///
/// Documented hashes are 32 hex digits with an optional `a_` prefix for
/// animated images (reference.mdx, "Image Formatting"); they are stored
/// compactly without a heap allocation. Any other string is kept as is, so
/// an unexpected hash format never makes a whole object fail to parse
/// (PLAN.md D14). Displays and serializes in the original form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageHash(Repr);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Repr {
    Md5 { bytes: [u8; 16], animated: bool },
    Other(Box<str>),
}

impl ImageHash {
    /// Parses a hash; never fails (unknown formats are kept as text).
    pub fn new(s: &str) -> Self {
        Self(parse_md5(s).unwrap_or_else(|| Repr::Other(s.into())))
    }

    /// Whether the image is animated (`a_` prefix).
    pub fn is_animated(&self) -> bool {
        match &self.0 {
            Repr::Md5 { animated, .. } => *animated,
            Repr::Other(s) => s.starts_with("a_"),
        }
    }

    /// The 16 raw bytes for a documented (hex) hash; `None` for other formats.
    pub const fn bytes(&self) -> Option<[u8; 16]> {
        match &self.0 {
            Repr::Md5 { bytes, .. } => Some(*bytes),
            Repr::Other(_) => None,
        }
    }
}

fn parse_md5(s: &str) -> Option<Repr> {
    let (animated, hex) = match s.strip_prefix("a_") {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let hex = hex.as_bytes();
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0; 16];
    let (pairs, _) = hex.as_chunks::<2>();
    for (byte, &[high, low]) in bytes.iter_mut().zip(pairs) {
        *byte = (nibble(high)? << 4) | nibble(low)?;
    }
    Some(Repr::Md5 { bytes, animated })
}

/// Lowercase hex only, so displaying gives back exactly the input.
fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

impl From<&str> for ImageHash {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl fmt::Display for ImageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let (bytes, animated) = match &self.0 {
            Repr::Md5 { bytes, animated } => (bytes, *animated),
            Repr::Other(s) => return f.write_str(s),
        };
        let mut out = [0u8; 34];
        let start = if animated {
            out[0] = b'a';
            out[1] = b'_';
            2
        } else {
            0
        };
        for (i, byte) in bytes.iter().enumerate() {
            out[start + i * 2] = HEX[usize::from(byte >> 4)];
            out[start + i * 2 + 1] = HEX[usize::from(byte & 0xF)];
        }
        // Only ASCII was written, so this never fails.
        let s = std::str::from_utf8(&out[..start + 32]).map_err(|_| fmt::Error)?;
        f.write_str(s)
    }
}

impl Serialize for ImageHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ImageHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;

        impl Visitor<'_> for V {
            type Value = ImageHash;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an image hash string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ImageHash, E> {
                Ok(ImageHash::new(v))
            }
        }

        deserializer.deserialize_str(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_docs_example() {
        let s = "a_1269e74af4df7417b13759eae50c83dc";
        let hash = ImageHash::new(s);
        assert!(hash.is_animated());
        assert!(hash.bytes().is_some());
        assert_eq!(hash.to_string(), s);

        let plain = ImageHash::new("1269e74af4df7417b13759eae50c83dc");
        assert!(!plain.is_animated());
        assert_eq!(plain.bytes(), hash.bytes());
    }

    #[test]
    fn serde_round_trip() {
        let json = "\"a_1269e74af4df7417b13759eae50c83dc\"";
        let hash: ImageHash = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&hash).unwrap(), json);
    }

    #[test]
    fn unexpected_formats_are_kept_not_rejected() {
        for other in [
            "",
            "1269e74af4df7417b13759eae50c83d",
            "1269E74AF4DF7417B13759EAE50C83DC",
            "nameplates/nameplates/twilight/",
            "a_short",
        ] {
            let hash = ImageHash::new(other);
            assert_eq!(hash.bytes(), None, "{other}");
            assert_eq!(hash.to_string(), other);
            let json = serde_json::to_string(&hash).unwrap();
            assert_eq!(serde_json::from_str::<ImageHash>(&json).unwrap(), hash);
        }
        assert!(ImageHash::new("a_short").is_animated());
    }

    #[test]
    fn documented_hash_needs_no_heap() {
        assert!(size_of::<ImageHash>() <= 24);
    }
}
