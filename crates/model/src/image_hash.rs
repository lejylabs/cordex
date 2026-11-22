//! CDN image hashes (avatars, icons, banners, …).

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;

/// An image hash such as `a_1269e74af4df7417b13759eae50c83dc`, stored in
/// 17 bytes instead of a heap string.
///
/// Hashes are 32 hex digits; the `a_` prefix marks an animated image
/// (reference.mdx, "Image Formatting"). Displays in the original form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHash {
    bytes: [u8; 16],
    animated: bool,
}

impl ImageHash {
    /// Whether the image is animated (`a_` prefix).
    pub const fn is_animated(self) -> bool {
        self.animated
    }

    /// The 16 raw bytes of the hash.
    pub const fn bytes(self) -> [u8; 16] {
        self.bytes
    }
}

impl FromStr for ImageHash {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (animated, hex) = match s.strip_prefix("a_") {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let hex = hex.as_bytes();
        if hex.len() != 32 {
            return Err(ParseError::ImageHash);
        }
        let mut bytes = [0; 16];
        let (pairs, _) = hex.as_chunks::<2>();
        for (byte, &[high, low]) in bytes.iter_mut().zip(pairs) {
            *byte = (nibble(high)? << 4) | nibble(low)?;
        }
        Ok(Self { bytes, animated })
    }
}

fn nibble(c: u8) -> Result<u8, ParseError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        // Discord sends lowercase; accept uppercase too, display lowercase.
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(ParseError::ImageHash),
    }
}

impl fmt::Display for ImageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0u8; 34];
        let start = if self.animated {
            out[0] = b'a';
            out[1] = b'_';
            2
        } else {
            0
        };
        for (i, byte) in self.bytes.iter().enumerate() {
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
                f.write_str("an image hash")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ImageHash, E> {
                v.parse()
                    .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
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
        let hash: ImageHash = s.parse().unwrap();
        assert!(hash.is_animated());
        assert_eq!(hash.to_string(), s);

        let plain: ImageHash = "1269e74af4df7417b13759eae50c83dc".parse().unwrap();
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
    fn rejects_bad_hashes() {
        for bad in [
            "",
            "a_",
            "1269e74af4df7417b13759eae50c83d",
            "1269e74af4df7417b13759eae50c83dcc",
            "1269e74af4df7417b13759eae50c83dg",
            "b_1269e74af4df7417b13759eae50c83dc",
        ] {
            assert_eq!(
                bad.parse::<ImageHash>(),
                Err(ParseError::ImageHash),
                "{bad}"
            );
        }
    }
}
