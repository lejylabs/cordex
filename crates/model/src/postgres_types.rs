//! Opt-in PostgreSQL `ToSql` / `FromSql` implementations (`postgres-types` feature).
//!
//! Supported mappings (binary wire format, zero `sqlx` overhead):
//! - [`Id<T>`] ↔ `INT8` (`BIGINT`, bit-preserving 8-byte big-endian; `0` rejected on decode)
//! - [`Permissions`] ↔ `INT8` (`BIGINT`, 64-bit bitfield)
//! - [`Timestamp`] ↔ `TIMESTAMPTZ` (8-byte big-endian `i64` microseconds since `2000-01-01T00:00:00Z`)
//! - [`ImageHash`] ↔ `TEXT` / `VARCHAR`

use std::error::Error;

use bytes::BytesMut;
use postgres_types::{FromSql, IsNull, ToSql, Type, to_sql_checked};

use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;

/// Offset between Unix epoch (`1970-01-01T00:00:00Z`) and Postgres epoch
/// (`2000-01-01T00:00:00Z`) in microseconds: `946_684_800 * 1_000_000`.
const POSTGRES_EPOCH_UNIX_MICROS: i64 = 946_684_800_000_000;

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

impl ToSql for Permissions {
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

impl<'a> FromSql<'a> for Permissions {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let arr: [u8; 8] = raw
            .try_into()
            .map_err(|_| "invalid INT8 length for Permissions")?;
        Ok(Self::from_bits_retain(u64::from_be_bytes(arr)))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

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
    fn permissions_roundtrip() {
        let perms = Permissions::ADMINISTRATOR | Permissions::BAN_MEMBERS;
        let mut buf = BytesMut::new();
        assert!(perms.to_sql_checked(&Type::INT8, &mut buf).is_ok());
        let decoded = Permissions::from_sql(&Type::INT8, &buf);
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert_eq!(decoded, perms);
        }
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
