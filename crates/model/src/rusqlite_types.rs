//! Opt-in SQLite (`rusqlite`) `ToSql` / `FromSql` implementations (`rusqlite` feature).
//!
//! - [`Id<T>`] ↔ `INTEGER` (bit-preserving `i64`; `0` rejected on decode)
//! - [`Permissions`] ↔ `INTEGER` (64-bit bitfield as `i64`)
//! - [`Timestamp`] ↔ `INTEGER` (Unix microseconds as `i64`)
//! - [`ImageHash`] ↔ `TEXT`

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::id::Id;
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::timestamp::Timestamp;

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

impl ToSql for Permissions {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(i64::from_ne_bytes(
            self.bits().to_ne_bytes(),
        )))
    }
}

impl FromSql for Permissions {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let raw = value.as_i64()?;
        Ok(Self::from_bits_retain(u64::from_ne_bytes(
            raw.to_ne_bytes(),
        )))
    }
}

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
