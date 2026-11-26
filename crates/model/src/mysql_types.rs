//! Opt-in MySQL / MariaDB (`mysql_common`) value conversions (`mysql` feature).
//!
//! Works seamlessly with both synchronous `mysql` and asynchronous `mysql_async`.
//! - [`Id<T>`] ↔ `UInt(u64)` / `Int(i64)` / `Bytes` (`0` rejected on decode)
//! - [`Permissions`] ↔ `UInt(u64)` / `Int(i64)`
//! - [`Timestamp`] ↔ `Int(i64)` (Unix microseconds)
//! - [`ImageHash`] ↔ `Bytes` (UTF-8 string)

use mysql_common::value::Value;
use mysql_common::value::convert::{FromValue, FromValueError};

use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;

impl<T> From<Id<T>> for Value {
    fn from(id: Id<T>) -> Self {
        Self::UInt(id.get())
    }
}

impl<T> FromValue for Id<T> {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::try_from(v).unwrap_or_else(|_| Self::from(std::num::NonZeroU64::MIN))
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        Self::try_from(v)
    }
}

impl<T> TryFrom<Value> for Id<T> {
    type Error = FromValueError;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        let raw = u64::from_value_opt(v.clone())?;
        Self::new(raw).ok_or(FromValueError(v))
    }
}

impl From<Permissions> for Value {
    fn from(perms: Permissions) -> Self {
        Self::UInt(perms.bits())
    }
}

impl FromValue for Permissions {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::from_value_opt(v).unwrap_or_else(|_| Self::empty())
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        let bits = u64::from_value_opt(v)?;
        Ok(Self::from_bits_retain(bits))
    }
}

impl From<Timestamp> for Value {
    fn from(ts: Timestamp) -> Self {
        Self::Int(ts.as_micros())
    }
}

impl FromValue for Timestamp {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::from_value_opt(v).unwrap_or_else(|_| Self::from_micros(0))
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        let micros = i64::from_value_opt(v)?;
        Ok(Self::from_micros(micros))
    }
}

impl From<ImageHash> for Value {
    fn from(hash: ImageHash) -> Self {
        Self::Bytes(hash.to_string().into_bytes())
    }
}

impl FromValue for ImageHash {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::from_value_opt(v).unwrap_or_else(|_| Self::new(""))
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        let s = String::from_value_opt(v)?;
        Ok(Self::new(&s))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;
    use crate::id::GuildId;

    #[test]
    fn mysql_roundtrip() {
        let id =
            GuildId::new(123_456_789_012_345_678).unwrap_or_else(|| GuildId::from(NonZeroU64::MIN));
        let val: Value = id.into();
        assert_eq!(GuildId::from_value_opt(val).ok(), Some(id));
        assert!(GuildId::from_value_opt(Value::UInt(0)).is_err());

        let perms = Permissions::MANAGE_GUILD;
        let p_val: Value = perms.into();
        assert_eq!(Permissions::from_value_opt(p_val).ok(), Some(perms));

        let ts = Timestamp::from_micros(1_700_000_000_000_000);
        let t_val: Value = ts.into();
        assert_eq!(Timestamp::from_value_opt(t_val).ok(), Some(ts));

        let hash = ImageHash::new("a_1234567890abcdef1234567890abcdef");
        let h_val: Value = hash.clone().into();
        assert_eq!(ImageHash::from_value_opt(h_val).ok(), Some(hash));
    }
}
