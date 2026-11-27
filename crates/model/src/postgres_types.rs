//! Opt-in PostgreSQL `ToSql` / `FromSql` implementations (`postgres-types` feature).
//!
//! Supported mappings (binary wire format, zero `sqlx` overhead):
//! - [`Id<T>`] ↔ `INT8` (`BIGINT`, bit-preserving 8-byte big-endian; `0` rejected on decode)
//! - Bitflags ([`Permissions`], [`Intents`], [`UserFlags`], [`MessageFlags`], [`ChannelFlags`],
//!   [`MemberFlags`], [`RoleFlags`], [`SystemChannelFlags`]) ↔ `INT8` (`BIGINT`, 64-bit bitfield)
//! - [`Timestamp`] ↔ `TIMESTAMPTZ` (8-byte big-endian `i64` microseconds since `2000-01-01T00:00:00Z`)
//! - [`ImageHash`] ↔ `TEXT` / `VARCHAR`
//! - [`Locale`] ↔ `TEXT` / `VARCHAR`
//! - Enums ([`ChannelType`], [`InteractionType`], [`PremiumTier`], [`AutoModerationActionType`])
//!   ↔ `INT2` (`SMALLINT`, `0..=255` validated on decode)
//! - JSON models ([`Embed`], [`Component`], [`AllowedMentions`]) ↔ `JSONB` / `JSON`

use std::error::Error;

use bytes::BytesMut;
use postgres_types::{FromSql, IsNull, ToSql, Type, to_sql_checked};

use crate::auto_moderation::AutoModerationActionType;
use crate::channel::{ChannelFlags, ChannelType};
use crate::component::Component;
use crate::gateway::Intents;
use crate::guild::{MemberFlags, PremiumTier, RoleFlags, SystemChannelFlags};
use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::interaction::InteractionType;
use crate::locale::Locale;
use crate::message::{AllowedMentions, Embed, MessageFlags};
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;
use crate::user::UserFlags;

/// Offset between Unix epoch (`1970-01-01T00:00:00Z`) and Postgres epoch
/// (`2000-01-01T00:00:00Z`) in microseconds: `946_684_800 * 1_000_000`.
const POSTGRES_EPOCH_UNIX_MICROS: i64 = 946_684_800_000_000;

/// Postgres binary `JSONB` wire format version prefix.
const JSONB_VERSION: u8 = 1;

impl<T: Sync + std::fmt::Debug> ToSql for Id<T> {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(&self.get().to_be_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }

    to_sql_checked!();
}

impl<'a, T> FromSql<'a> for Id<T> {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let arr: [u8; 8] = raw.try_into().map_err(|_| "invalid INT8 length for Id")?;
        let val = u64::from_be_bytes(arr);
        Self::new(val).ok_or_else(|| "snowflake Id cannot be zero".into())
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

macro_rules! impl_pg_bitflags {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToSql for $ty {
                fn to_sql(
                    &self,
                    _ty: &Type,
                    out: &mut BytesMut,
                ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
                    out.extend_from_slice(&self.bits().to_be_bytes());
                    Ok(IsNull::No)
                }

                fn accepts(ty: &Type) -> bool {
                    *ty == Type::INT8
                }

                to_sql_checked!();
            }

            impl<'a> FromSql<'a> for $ty {
                fn from_sql(
                    _ty: &Type,
                    raw: &'a [u8],
                ) -> Result<Self, Box<dyn Error + Sync + Send>> {
                    let arr: [u8; 8] = raw
                        .try_into()
                        .map_err(|_| concat!("invalid INT8 length for ", stringify!($ty)))?;
                    Ok(Self::from_bits_retain(u64::from_be_bytes(arr)))
                }

                fn accepts(ty: &Type) -> bool {
                    *ty == Type::INT8
                }
            }
        )+
    };
}

impl_pg_bitflags!(
    Permissions,
    Intents,
    UserFlags,
    MessageFlags,
    ChannelFlags,
    MemberFlags,
    RoleFlags,
    SystemChannelFlags,
);

macro_rules! impl_pg_u8_enum {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToSql for $ty {
                fn to_sql(
                    &self,
                    _ty: &Type,
                    out: &mut BytesMut,
                ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
                    let val = i16::from(u8::from(*self));
                    out.extend_from_slice(&val.to_be_bytes());
                    Ok(IsNull::No)
                }

                fn accepts(ty: &Type) -> bool {
                    *ty == Type::INT2
                }

                to_sql_checked!();
            }

            impl<'a> FromSql<'a> for $ty {
                fn from_sql(
                    _ty: &Type,
                    raw: &'a [u8],
                ) -> Result<Self, Box<dyn Error + Sync + Send>> {
                    let arr: [u8; 2] = raw
                        .try_into()
                        .map_err(|_| concat!("invalid INT2 length for ", stringify!($ty)))?;
                    let val = i16::from_be_bytes(arr);
                    let byte = u8::try_from(val).map_err(|_| {
                        concat!(stringify!($ty), " value out of u8 range (0..=255)")
                    })?;
                    Ok(Self::from(byte))
                }

                fn accepts(ty: &Type) -> bool {
                    *ty == Type::INT2
                }
            }
        )+
    };
}

impl_pg_u8_enum!(
    ChannelType,
    InteractionType,
    PremiumTier,
    AutoModerationActionType,
);

macro_rules! impl_pg_json {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ToSql for $ty {
                fn to_sql(
                    &self,
                    ty: &Type,
                    out: &mut BytesMut,
                ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
                    if *ty == Type::JSONB {
                        out.extend_from_slice(&[JSONB_VERSION]);
                    }
                    let bytes = serde_json::to_vec(self)?;
                    out.extend_from_slice(&bytes);
                    Ok(IsNull::No)
                }

                fn accepts(ty: &Type) -> bool {
                    matches!(*ty, Type::JSONB | Type::JSON)
                }

                to_sql_checked!();
            }

            impl<'a> FromSql<'a> for $ty {
                fn from_sql(
                    ty: &Type,
                    raw: &'a [u8],
                ) -> Result<Self, Box<dyn Error + Sync + Send>> {
                    let json_bytes = if *ty == Type::JSONB {
                        let (&version, rest) = raw
                            .split_first()
                            .ok_or("empty JSONB payload")?;
                        if version != JSONB_VERSION {
                            return Err(format!("unsupported JSONB version: {version}").into());
                        }
                        rest
                    } else {
                        raw
                    };
                    Ok(serde_json::from_slice(json_bytes)?)
                }

                fn accepts(ty: &Type) -> bool {
                    matches!(*ty, Type::JSONB | Type::JSON)
                }
            }
        )+
    };
}

impl_pg_json!(Embed, Component, AllowedMentions);

impl ToSql for Timestamp {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let pg_micros = self
            .as_micros()
            .checked_sub(POSTGRES_EPOCH_UNIX_MICROS)
            .ok_or("timestamp out of PostgreSQL TIMESTAMPTZ range")?;
        out.extend_from_slice(&pg_micros.to_be_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TIMESTAMPTZ
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for Timestamp {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let arr: [u8; 8] = raw
            .try_into()
            .map_err(|_| "invalid TIMESTAMPTZ length for Timestamp")?;
        let pg_micros = i64::from_be_bytes(arr);
        let unix_micros = pg_micros
            .checked_add(POSTGRES_EPOCH_UNIX_MICROS)
            .ok_or("PostgreSQL TIMESTAMPTZ overflows Timestamp")?;
        Ok(Self::from_micros(unix_micros))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TIMESTAMPTZ
    }
}

impl ToSql for ImageHash {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(self.to_string().as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::TEXT | Type::VARCHAR)
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for ImageHash {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = std::str::from_utf8(raw)?;
        Ok(Self::new(s))
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::TEXT | Type::VARCHAR)
    }
}

impl ToSql for Locale {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(self.as_str().as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::TEXT | Type::VARCHAR)
    }

    to_sql_checked!();
}

impl<'a> FromSql<'a> for Locale {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = std::str::from_utf8(raw)?;
        Ok(Self::from_code(s))
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::TEXT | Type::VARCHAR)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;
    use crate::id::GuildId;

    #[test]
    fn id_roundtrip_and_zero_rejection() {
        let id =
            GuildId::new(123_456_789_012_345_678).unwrap_or_else(|| GuildId::from(NonZeroU64::MIN));
        let mut buf = BytesMut::new();
        assert!(id.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(buf.len(), 8);
        let decoded = GuildId::from_sql(&Type::INT8, &buf);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded, id);
        }

        // Zero must be rejected
        assert!(GuildId::from_sql(&Type::INT8, &[0u8; 8]).is_err());
        // Wrong Postgres type rejected
        assert!(!<GuildId as ToSql>::accepts(&Type::TEXT));
    }

    #[test]
    fn bitflags_roundtrip() {
        let perms = Permissions::ADMINISTRATOR | Permissions::BAN_MEMBERS;
        let mut buf = BytesMut::new();
        assert!(perms.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(Permissions::from_sql(&Type::INT8, &buf).ok(), Some(perms));

        let intents = Intents::GUILDS | Intents::GUILD_MESSAGES;
        buf.clear();
        assert!(intents.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(Intents::from_sql(&Type::INT8, &buf).ok(), Some(intents));

        let mflags = MessageFlags::EPHEMERAL | MessageFlags::SUPPRESS_EMBEDS;
        buf.clear();
        assert!(mflags.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(MessageFlags::from_sql(&Type::INT8, &buf).ok(), Some(mflags));

        let uflags = UserFlags::STAFF | UserFlags::VERIFIED_BOT;
        buf.clear();
        assert!(uflags.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(UserFlags::from_sql(&Type::INT8, &buf).ok(), Some(uflags));

        let cflags = ChannelFlags::PINNED;
        buf.clear();
        assert!(cflags.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(ChannelFlags::from_sql(&Type::INT8, &buf).ok(), Some(cflags));

        let mem_flags = MemberFlags::COMPLETED_ONBOARDING;
        buf.clear();
        assert!(mem_flags.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        assert_eq!(
            MemberFlags::from_sql(&Type::INT8, &buf).ok(),
            Some(mem_flags)
        );
    }

    #[test]
    fn enum_roundtrip_and_range_check() {
        let mut buf = BytesMut::new();
        assert!(
            ChannelType::GuildForum
                .to_sql_checked(&Type::INT2, &mut buf)
                .is_ok()
        );
        assert_eq!(
            ChannelType::from_sql(&Type::INT2, &buf).ok(),
            Some(ChannelType::GuildForum)
        );

        buf.clear();
        assert!(
            InteractionType::ApplicationCommand
                .to_sql_checked(&Type::INT2, &mut buf)
                .is_ok()
        );
        assert_eq!(
            InteractionType::from_sql(&Type::INT2, &buf).ok(),
            Some(InteractionType::ApplicationCommand)
        );

        buf.clear();
        assert!(
            PremiumTier::Tier3
                .to_sql_checked(&Type::INT2, &mut buf)
                .is_ok()
        );
        assert_eq!(
            PremiumTier::from_sql(&Type::INT2, &buf).ok(),
            Some(PremiumTier::Tier3)
        );

        buf.clear();
        assert!(
            AutoModerationActionType::Timeout
                .to_sql_checked(&Type::INT2, &mut buf)
                .is_ok()
        );
        assert_eq!(
            AutoModerationActionType::from_sql(&Type::INT2, &buf).ok(),
            Some(AutoModerationActionType::Timeout)
        );

        // Negative or > 255 must be rejected
        assert!(ChannelType::from_sql(&Type::INT2, &(-1i16).to_be_bytes()).is_err());
        assert!(ChannelType::from_sql(&Type::INT2, &256i16.to_be_bytes()).is_err());
    }

    #[test]
    fn locale_roundtrip() {
        let mut buf = BytesMut::new();
        assert!(Locale::De.to_sql_checked(&Type::TEXT, &mut buf).is_ok());
        assert_eq!(Locale::from_sql(&Type::TEXT, &buf).ok(), Some(Locale::De));

        let unknown = Locale::from_code("xx-YY");
        buf.clear();
        assert!(unknown.to_sql_checked(&Type::VARCHAR, &mut buf).is_ok());
        assert_eq!(Locale::from_sql(&Type::VARCHAR, &buf).ok(), Some(unknown));
    }

    #[test]
    fn jsonb_and_json_roundtrip() {
        let mentions = AllowedMentions::none();
        let mut buf = BytesMut::new();
        assert!(mentions.to_sql_checked(&Type::JSONB, &mut buf).is_ok());
        assert_eq!(buf.first().copied(), Some(JSONB_VERSION));
        assert_eq!(
            AllowedMentions::from_sql(&Type::JSONB, &buf).ok(),
            Some(mentions.clone())
        );

        buf.clear();
        assert!(mentions.to_sql_checked(&Type::JSON, &mut buf).is_ok());
        assert_eq!(
            AllowedMentions::from_sql(&Type::JSON, &buf).ok(),
            Some(mentions)
        );

        // Wrong JSONB version rejected
        assert!(AllowedMentions::from_sql(&Type::JSONB, &[2, b'{', b'}']).is_err());
    }

    #[test]
    fn timestamp_roundtrip_and_overflow() {
        // Postgres epoch 2000-01-01T00:00:00Z encodes as 0
        let pg_epoch = Timestamp::from_micros(POSTGRES_EPOCH_UNIX_MICROS);
        let mut buf = BytesMut::new();
        assert!(
            pg_epoch
                .to_sql_checked(&Type::TIMESTAMPTZ, &mut buf)
                .is_ok()
        );
        assert_eq!(&buf[..], &0i64.to_be_bytes());
        let decoded = Timestamp::from_sql(&Type::TIMESTAMPTZ, &buf);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded, pg_epoch);
        }

        // Overflow check
        let max_pg = i64::MAX.to_be_bytes();
        assert!(Timestamp::from_sql(&Type::TIMESTAMPTZ, &max_pg).is_err());
    }

    #[test]
    fn image_hash_roundtrip() {
        let hash = ImageHash::new("a_1234567890abcdef1234567890abcdef");
        let mut buf = BytesMut::new();
        assert!(hash.to_sql_checked(&Type::TEXT, &mut buf).is_ok());
        let decoded = ImageHash::from_sql(&Type::TEXT, &buf);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded, hash);
        }
    }
}
