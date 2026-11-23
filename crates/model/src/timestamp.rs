//! ISO 8601 timestamps.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::ParseError;

/// Display style of a Discord timestamp mention (`<t:unix:style>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TimestampStyle {
    /// Short time (`16:20`).
    ShortTime,
    /// Long time (`16:20:30`).
    LongTime,
    /// Short date (`20/04/2021`).
    ShortDate,
    /// Long date (`20 April 2021`).
    LongDate,
    /// Short date and time (`20 April 2021 16:20`).
    ShortDateTime,
    /// Long date and time (`Tuesday, 20 April 2021 16:20`).
    LongDateTime,
    /// Relative (`2 months ago`).
    Relative,
}

impl TimestampStyle {
    /// Style character Discord uses after the unix timestamp.
    pub const fn as_char(self) -> char {
        match self {
            Self::ShortTime => 't',
            Self::LongTime => 'T',
            Self::ShortDate => 'd',
            Self::LongDate => 'D',
            Self::ShortDateTime => 'f',
            Self::LongDateTime => 'F',
            Self::Relative => 'R',
        }
    }

    /// Parse a style character; `None` if unknown.
    pub const fn from_char(c: u8) -> Option<Self> {
        match c {
            b't' => Some(Self::ShortTime),
            b'T' => Some(Self::LongTime),
            b'd' => Some(Self::ShortDate),
            b'D' => Some(Self::LongDate),
            b'f' => Some(Self::ShortDateTime),
            b'F' => Some(Self::LongDateTime),
            b'R' => Some(Self::Relative),
            _ => None,
        }
    }
}

/// A point in time, stored as Unix microseconds (8 bytes).
///
/// Discord sends ISO 8601 strings such as `2017-07-11T17:27:07.299000+00:00`
/// (reference.mdx, "ISO8601 Date/Time"); they are parsed as RFC 3339.
/// Serializes back to RFC 3339 in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    micros: i64,
}

impl Timestamp {
    /// From Unix microseconds.
    pub const fn from_micros(micros: i64) -> Self {
        Self { micros }
    }

    /// From Unix milliseconds (e.g. snowflake timestamps).
    pub const fn from_millis(millis: i64) -> Self {
        Self {
            micros: millis.saturating_mul(1000),
        }
    }

    /// Unix microseconds.
    pub const fn as_micros(self) -> i64 {
        self.micros
    }

    /// Unix milliseconds (rounded down).
    pub const fn as_millis(self) -> i64 {
        self.micros.div_euclid(1000)
    }

    /// Unix seconds (rounded down).
    pub const fn as_secs(self) -> i64 {
        self.micros.div_euclid(1_000_000)
    }

    /// Discord timestamp markdown (`<t:unix:style>`).
    pub fn markdown(self, style: TimestampStyle) -> String {
        format!("<t:{}:{}>", self.as_secs(), style.as_char())
    }

    /// Discord timestamp markdown with the default style (`<t:unix>`).
    pub fn markdown_default(self) -> String {
        format!("<t:{}>", self.as_secs())
    }

    /// As a `time` value in UTC; `None` outside `time`'s supported range.
    pub fn to_offset_date_time(self) -> Option<OffsetDateTime> {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(self.micros) * 1000).ok()
    }

    /// From a `time` value; `None` if it does not fit into `i64` microseconds.
    pub fn from_offset_date_time(value: OffsetDateTime) -> Option<Self> {
        i64::try_from(value.unix_timestamp_nanos() / 1000)
            .ok()
            .map(Self::from_micros)
    }
}

impl FromStr for Timestamp {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        OffsetDateTime::parse(s, &Rfc3339)
            .ok()
            .and_then(Self::from_offset_date_time)
            .ok_or(ParseError::Timestamp)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted = self
            .to_offset_date_time()
            .and_then(|t| t.format(&Rfc3339).ok())
            .ok_or(fmt::Error)?;
        f.write_str(&formatted)
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;

        impl Visitor<'_> for V {
            type Value = Timestamp;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an ISO 8601 timestamp")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Timestamp, E> {
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
    fn parses_discord_formats() {
        // With microseconds (most resources), without fraction, and with `Z`.
        let a: Timestamp = "2017-07-11T17:27:07.299000+00:00".parse().unwrap();
        assert_eq!(a.as_millis(), 1_499_794_027_299);
        let b: Timestamp = "2017-07-11T17:27:07+00:00".parse().unwrap();
        assert_eq!(b.as_secs(), 1_499_794_027);
        let c: Timestamp = "2017-07-11T17:27:07.299Z".parse().unwrap();
        assert_eq!(a, c);
    }

    #[test]
    fn other_offsets_are_normalized() {
        let a: Timestamp = "2017-07-11T19:27:07+02:00".parse().unwrap();
        let b: Timestamp = "2017-07-11T17:27:07+00:00".parse().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_round_trip() {
        let t: Timestamp = serde_json::from_str("\"2017-07-11T17:27:07.299000+00:00\"").unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"2017-07-11T17:27:07.299Z\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), t);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["\"\"", "\"yesterday\"", "\"2017-07-11\"", "1499794027"] {
            assert!(serde_json::from_str::<Timestamp>(bad).is_err(), "{bad}");
        }
    }
}
