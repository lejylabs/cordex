//! Errors when parsing model values from strings.

/// A string could not be parsed into a model value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// Not a valid snowflake (decimal `u64`, not zero).
    #[error("invalid snowflake")]
    Snowflake,
    /// Not a valid image hash (optional `a_` prefix plus 32 hex digits).
    #[error("invalid image hash")]
    ImageHash,
    /// Not a valid ISO 8601 / RFC 3339 timestamp.
    #[error("invalid timestamp")]
    Timestamp,
    /// Not a valid bitfield (decimal `u64`).
    #[error("invalid bitfield")]
    Bitfield,
}
