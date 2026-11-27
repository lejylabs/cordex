//! Opt-in MySQL / MariaDB (`mysql_common`) value conversions (`mysql` feature).
//!
//! Works seamlessly with both synchronous `mysql` and asynchronous `mysql_async`.
//! - [`Id<T>`] ↔ `UInt(u64)` / `Int(i64)` / `Bytes` (`0` rejected on decode)
//! - Bitflags ([`Permissions`], [`Intents`], [`UserFlags`], [`MessageFlags`], [`ChannelFlags`],
//!   [`MemberFlags`], [`RoleFlags`], [`SystemChannelFlags`]) ↔ `UInt(u64)` / `Int(i64)`
//! - [`Timestamp`] ↔ `Int(i64)` (Unix microseconds)
//! - [`ImageHash`] ↔ `Bytes` (UTF-8 string)
//! - [`Locale`] ↔ `Bytes` (UTF-8 string)
//! - Enums ([`ChannelType`], [`InteractionType`], [`PremiumTier`], [`AutoModerationActionType`])
//!   ↔ `UInt` / `Int` (`0..=255`)

use mysql_common::value::Value;
use mysql_common::value::convert::{FromValue, FromValueError};

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

macro_rules! impl_mysql_bitflags {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for Value {
                fn from(flags: $ty) -> Self {
                    Self::UInt(flags.bits())
                }
            }

            impl FromValue for $ty {
                type Intermediate = Self;

                fn from_value(v: Value) -> Self {
                    Self::try_from(v).unwrap_or_else(|_| Self::empty())
                }

                fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
                    Self::try_from(v)
                }
            }

            impl TryFrom<Value> for $ty {
                type Error = FromValueError;

                fn try_from(v: Value) -> Result<Self, Self::Error> {
                    let bits = u64::from_value_opt(v)?;
                    Ok(Self::from_bits_retain(bits))
                }
            }
        )+
    };
}

impl_mysql_bitflags!(
    Permissions,
    Intents,
    UserFlags,
    MessageFlags,
    ChannelFlags,
    MemberFlags,
    RoleFlags,
    SystemChannelFlags,
);

macro_rules! impl_mysql_u8_enum {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for Value {
                fn from(val: $ty) -> Self {
                    Self::UInt(u64::from(u8::from(val)))
                }
            }

            impl FromValue for $ty {
                type Intermediate = Self;

                fn from_value(v: Value) -> Self {
                    Self::try_from(v).unwrap_or_else(|_| Self::from(0))
                }

                fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
                    Self::try_from(v)
                }
            }

            impl TryFrom<Value> for $ty {
                type Error = FromValueError;

                fn try_from(v: Value) -> Result<Self, Self::Error> {
                    let raw = u8::from_value_opt(v)?;
                    Ok(Self::from(raw))
                }
            }
        )+
    };
}

impl_mysql_u8_enum!(
    ChannelType,
    InteractionType,
    PremiumTier,
    AutoModerationActionType,
);

impl From<Timestamp> for Value {
    fn from(ts: Timestamp) -> Self {
        Self::Int(ts.as_micros())
    }
}

impl FromValue for Timestamp {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::try_from(v).unwrap_or_else(|_| Self::from_micros(0))
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        Self::try_from(v)
    }
}

impl TryFrom<Value> for Timestamp {
    type Error = FromValueError;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
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
        Self::try_from(v).unwrap_or_else(|_| Self::new(""))
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        Self::try_from(v)
    }
}

impl TryFrom<Value> for ImageHash {
    type Error = FromValueError;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        let s = String::from_value_opt(v)?;
        Ok(Self::new(&s))
    }
}

impl From<Locale> for Value {
    fn from(locale: Locale) -> Self {
        Self::Bytes(locale.as_str().as_bytes().to_vec())
    }
}

impl FromValue for Locale {
    type Intermediate = Self;

    fn from_value(v: Value) -> Self {
        Self::try_from(v).unwrap_or(Self::EnUs)
    }

    fn from_value_opt(v: Value) -> Result<Self, FromValueError> {
        Self::try_from(v)
    }
}

impl TryFrom<Value> for Locale {
    type Error = FromValueError;

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        let s = String::from_value_opt(v)?;
        Ok(Self::from_code(&s))
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

        let loc_val: Value = Locale::De.into();
        assert_eq!(Locale::from_value_opt(loc_val).ok(), Some(Locale::De));

        let ct_val: Value = ChannelType::GuildText.into();
        assert_eq!(
            ChannelType::from_value_opt(ct_val).ok(),
            Some(ChannelType::GuildText)
        );
    }
}
