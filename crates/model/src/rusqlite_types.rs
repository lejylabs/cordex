//! Opt-in SQLite (`rusqlite`) `ToSql` / `FromSql` implementations (`rusqlite` feature).
//!
//! - [`Id<T>`] ↔ `INTEGER` (bit-preserving `i64`; `0` rejected on decode)
//! - Bitflags ([`Permissions`], [`Intents`], [`UserFlags`], [`MessageFlags`], [`ChannelFlags`],
//!   [`MemberFlags`], [`RoleFlags`], [`SystemChannelFlags`]) ↔ `INTEGER` (64-bit bitfield as `i64`)
//! - [`Timestamp`] ↔ `INTEGER` (Unix microseconds as `i64`)
//! - [`ImageHash`] ↔ `TEXT`
//! - [`Locale`] ↔ `TEXT`
//! - Enums ([`ChannelType`], [`InteractionType`], [`PremiumTier`], [`AutoModerationActionType`])
//!   ↔ `INTEGER` (`0..=255` validated on decode)

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

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

impl<T> ToSql for Id<T> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(i64::from_ne_bytes(
            self.get().to_ne_bytes(),
        )))
    }
}

impl<T> FromSql for Id<T> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let raw = value.as_i64()?;
        let val = u64::from_ne_bytes(raw.to_ne_bytes());
        Self::new(val).ok_or_else(|| FromSqlError::Other("snowflake Id cannot be zero".into()))
    }
}

macro_rules! impl_sqlite_bitflags {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToSql for $ty {
                fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                    Ok(ToSqlOutput::from(i64::from_ne_bytes(
                        self.bits().to_ne_bytes(),
                    )))
                }
            }

            impl FromSql for $ty {
                fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                    let raw = value.as_i64()?;
                    Ok(Self::from_bits_retain(u64::from_ne_bytes(
                        raw.to_ne_bytes(),
                    )))
                }
            }
        )+
    };
}

impl_sqlite_bitflags!(
    Permissions,
    Intents,
    UserFlags,
    MessageFlags,
    ChannelFlags,
    MemberFlags,
    RoleFlags,
    SystemChannelFlags,
);

macro_rules! impl_sqlite_u8_enum {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToSql for $ty {
                fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                    Ok(ToSqlOutput::from(i64::from(u8::from(*self))))
                }
            }

            impl FromSql for $ty {
                fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                    let raw = value.as_i64()?;
                    let byte = u8::try_from(raw).map_err(|_| {
                        FromSqlError::Other(
                            concat!(stringify!($ty), " value out of u8 range (0..=255)").into(),
                        )
                    })?;
                    Ok(Self::from(byte))
                }
            }
        )+
    };
}

impl_sqlite_u8_enum!(
    ChannelType,
    InteractionType,
    PremiumTier,
    AutoModerationActionType,
);

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_micros()))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(Self::from_micros(value.as_i64()?))
    }
}

impl ToSql for ImageHash {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for ImageHash {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(Self::new(value.as_str()?))
    }
}

impl ToSql for Locale {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for Locale {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(Self::from_code(value.as_str()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_extended_types_roundtrip() {
        assert_eq!(
            Locale::column_result(ValueRef::Text(b"de")).ok(),
            Some(Locale::De)
        );
        assert_eq!(
            ChannelType::column_result(ValueRef::Integer(15)).ok(),
            Some(ChannelType::GuildForum)
        );
        assert!(ChannelType::column_result(ValueRef::Integer(-1)).is_err());
        assert!(ChannelType::column_result(ValueRef::Integer(256)).is_err());

        let intents = Intents::GUILDS | Intents::GUILD_MESSAGES;
        let encoded = i64::from_ne_bytes(intents.bits().to_ne_bytes());
        assert_eq!(
            Intents::column_result(ValueRef::Integer(encoded)).ok(),
            Some(intents)
        );
    }
}
