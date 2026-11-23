//! Stickers (resources/sticker.mdx, "Sticker Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{GuildMarker, StickerMarker, StickerPackMarker};
use crate::serde_util::int_enum;
use crate::user::User;

/// A standard or guild sticker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Sticker {
    /// Sticker ID.
    pub id: Id<StickerMarker>,
    /// Pack of a standard sticker.
    pub pack_id: Option<Id<StickerPackMarker>>,
    /// Sticker name.
    pub name: Box<str>,
    /// Description.
    pub description: Option<Box<str>>,
    /// Autocomplete tags (comma separated; unicode emoji name for guild stickers).
    #[serde(default)]
    pub tags: Box<str>,
    /// Sticker type.
    #[serde(rename = "type")]
    pub kind: StickerType,
    /// Format type.
    pub format_type: StickerFormatType,
    /// Whether a guild sticker can be used (false when boosts were lost).
    pub available: Option<bool>,
    /// Guild that owns the sticker.
    pub guild_id: Option<Id<GuildMarker>>,
    /// User that uploaded the guild sticker.
    pub user: Option<Box<User>>,
    /// Sort order within its pack.
    pub sort_value: Option<u32>,
}

int_enum! {
    /// Sticker type.
    pub enum StickerType: u8 {
        /// Official sticker in a pack.
        Standard = 1,
        /// Sticker uploaded to a guild.
        Guild = 2,
    }
}

int_enum! {
    /// Sticker format.
    pub enum StickerFormatType: u8 {
        /// PNG.
        Png = 1,
        /// Animated PNG.
        Apng = 2,
        /// Lottie (JSON animation).
        Lottie = 3,
        /// GIF.
        Gif = 4,
    }
}

/// Sticker as sent in messages (resources/sticker.mdx, "Sticker Item Object").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StickerItem {
    /// Sticker ID.
    pub id: Id<StickerMarker>,
    /// Name.
    pub name: Box<str>,
    /// Format type.
    pub format_type: StickerFormatType,
}

/// A pack of standard stickers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StickerPack {
    /// Pack ID.
    pub id: Id<StickerPackMarker>,
    /// Stickers in the pack.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
    /// Name.
    pub name: Box<str>,
    /// SKU of the pack.
    pub sku_id: Id<crate::id::marker::SkuMarker>,
    /// Sticker shown as the pack icon.
    pub cover_sticker_id: Option<Id<StickerMarker>>,
    /// Description.
    #[serde(default)]
    pub description: Box<str>,
    /// Banner image asset ID.
    pub banner_asset_id: Option<Id<crate::id::marker::GenericMarker>>,
}
