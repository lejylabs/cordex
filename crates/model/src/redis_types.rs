//! Opt-in Redis `ToRedisArgs` / `FromRedisValue` implementations (`redis` feature).
//!
//! - [`Id<T>`] ↔ decimal snowflake string / integer (`0` rejected on decode)
//! - [`Permissions`] ↔ `u64` bitfield
//! - [`Timestamp`] ↔ `i64` Unix microseconds
//! - [`ImageHash`] ↔ UTF-8 string

use redis::{FromRedisValue, ParsingError, RedisWrite, ToRedisArgs, Value};

use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;

impl<T> ToRedisArgs for Id<T> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.get().write_redis_args(out);
    }
}

impl<T> FromRedisValue for Id<T> {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let raw = u64::from_redis_value(v)?;
        Self::new(raw).ok_or_else(|| ParsingError::from("snowflake Id cannot be zero"))
    }
}

impl ToRedisArgs for Permissions {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.bits().write_redis_args(out);
    }
}

impl FromRedisValue for Permissions {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let bits = u64::from_redis_value(v)?;
        Ok(Self::from_bits_retain(bits))
    }
}

impl ToRedisArgs for Timestamp {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.as_micros().write_redis_args(out);
    }
}

impl FromRedisValue for Timestamp {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let micros = i64::from_redis_value(v)?;
        Ok(Self::from_micros(micros))
    }
}

impl ToRedisArgs for ImageHash {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.to_string().write_redis_args(out);
    }
}

impl FromRedisValue for ImageHash {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let s = String::from_redis_value(v)?;
        Ok(Self::new(&s))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;
    use crate::id::GuildId;

    #[test]
    fn redis_roundtrip() {
        let id =
            GuildId::new(123_456_789_012_345_678).unwrap_or_else(|| GuildId::from(NonZeroU64::MIN));
        let args = id.to_redis_args();
        assert_eq!(args.len(), 1);
        let decoded = GuildId::from_redis_value(Value::BulkString(args[0].clone()));
        assert_eq!(decoded.ok(), Some(id));
        assert!(GuildId::from_redis_value(Value::Int(0)).is_err());

        let perms = Permissions::BAN_MEMBERS | Permissions::KICK_MEMBERS;
        let p_args = perms.to_redis_args();
        assert_eq!(
            Permissions::from_redis_value(Value::BulkString(p_args[0].clone())).ok(),
            Some(perms)
        );
    }
}
