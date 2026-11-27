//! Opt-in Redis `ToRedisArgs` / `FromRedisValue` implementations (`redis` feature).
//!
//! - [`Id<T>`] ↔ decimal snowflake string / integer (`0` rejected on decode)
//! - Bitflags ([`Permissions`], [`Intents`], [`UserFlags`], [`MessageFlags`], [`ChannelFlags`],
//!   [`MemberFlags`], [`RoleFlags`], [`SystemChannelFlags`]) ↔ `u64` bitfield
//! - [`Timestamp`] ↔ `i64` Unix microseconds
//! - [`ImageHash`] ↔ UTF-8 string
//! - [`Locale`] ↔ UTF-8 string
//! - Enums ([`ChannelType`], [`InteractionType`], [`PremiumTier`], [`AutoModerationActionType`])
//!   ↔ `u8`

use redis::{FromRedisValue, ParsingError, RedisWrite, ToRedisArgs, Value};

use crate::auto_moderation::AutoModerationActionType;
use crate::channel::{ChannelFlags, ChannelType};
use crate::gateway::Intents;
use crate::guild::{MemberFlags, PremiumTier, RoleFlags, SystemChannelFlags};
use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::interaction::InteractionType;
use crate::locale::Locale;
use crate::message::MessageFlags;
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;
use crate::user::UserFlags;

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

macro_rules! impl_redis_bitflags {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToRedisArgs for $ty {
                fn write_redis_args<W>(&self, out: &mut W)
                where
                    W: ?Sized + RedisWrite,
                {
                    self.bits().write_redis_args(out);
                }
            }

            impl FromRedisValue for $ty {
                fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
                    let bits = u64::from_redis_value(v)?;
                    Ok(Self::from_bits_retain(bits))
                }
            }
        )+
    };
}

impl_redis_bitflags!(
    Permissions,
    Intents,
    UserFlags,
    MessageFlags,
    ChannelFlags,
    MemberFlags,
    RoleFlags,
    SystemChannelFlags,
);

macro_rules! impl_redis_u8_enum {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToRedisArgs for $ty {
                fn write_redis_args<W>(&self, out: &mut W)
                where
                    W: ?Sized + RedisWrite,
                {
                    u8::from(*self).write_redis_args(out);
                }
            }

            impl FromRedisValue for $ty {
                fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
                    let raw = u8::from_redis_value(v)?;
                    Ok(Self::from(raw))
                }
            }
        )+
    };
}

impl_redis_u8_enum!(
    ChannelType,
    InteractionType,
    PremiumTier,
    AutoModerationActionType,
);

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

impl ToRedisArgs for Locale {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        self.as_str().write_redis_args(out);
    }
}

impl FromRedisValue for Locale {
    fn from_redis_value(v: Value) -> Result<Self, ParsingError> {
        let s = String::from_redis_value(v)?;
        Ok(Self::from_code(&s))
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

        let loc_args = Locale::EnUs.to_redis_args();
        assert_eq!(
            Locale::from_redis_value(Value::BulkString(loc_args[0].clone())).ok(),
            Some(Locale::EnUs)
        );

        let ct_args = ChannelType::GuildVoice.to_redis_args();
        assert_eq!(
            ChannelType::from_redis_value(Value::BulkString(ct_args[0].clone())).ok(),
            Some(ChannelType::GuildVoice)
        );
    }
}
