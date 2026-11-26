//! Opt-in SIMD-accelerated JSON deserialization (`simd-json` feature).
//!
//! Uses SIMD instructions (AVX2 / SSE4.2 / NEON) to parse mutable UTF-8 JSON
//! byte slices 2–3× faster on large Discord payloads such as `GUILD_CREATE`.

use serde::de::DeserializeOwned;

/// Deserialize any `cordex-model` type from a mutable JSON byte buffer using `simd-json`.
pub fn from_slice<T: DeserializeOwned>(bytes: &mut [u8]) -> Result<T, simd_json::Error> {
    simd_json::serde::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user::User;

    #[test]
    fn parses_user_with_simd_json() {
        let mut raw =
            br#"{"id":"123456789012345678","username":"alice","discriminator":"0"}"#.to_vec();
        let parsed: Result<User, _> = from_slice(&mut raw);
        assert!(parsed.is_ok());
        if let Ok(user) = parsed {
            assert_eq!(user.username.as_ref(), "alice");
        }
    }
}
