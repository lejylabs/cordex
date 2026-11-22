//! Discord API v10 models: snowflakes, flags, REST resources and gateway
//! payloads (PLAN.md §5).
//!
//! No async or network dependencies; checked by `cargo xtask layout-check`.
//!
//! Reference: <https://github.com/discord/discord-api-docs> (the official
//! developer documentation wins over every other source).

pub mod error;
pub mod gateway;
pub mod id;
pub mod image_hash;
pub mod permissions;
pub mod timestamp;
pub mod token;

mod serde_util;

pub use error::ParseError;
pub use id::Id;
pub use image_hash::ImageHash;
pub use permissions::Permissions;
pub use timestamp::Timestamp;
pub use token::Token;

/// Discord API version this crate models.
pub const API_VERSION: u8 = 10;
