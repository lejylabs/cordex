//! Embeds (resources/message.mdx, "Embed Object").

use serde::{Deserialize, Serialize};

use crate::component::Component;
use crate::serde_util::{bitflags_serde_int, str_enum};
use crate::timestamp::Timestamp;

/// Rich content attached to a message.
///
/// Limits (docs, "Embed Limits"): title 256, description 4096, 25 fields
/// (name 256, value 1024), footer 2048, author name 256 characters, 6000
/// characters in total across all embeds of a message.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Embed {
    /// Title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Box<str>>,
    /// Type (always `rich` for webhook and bot embeds).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EmbedType>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// URL of the title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Box<str>>,
    /// Timestamp shown in the footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// Color (RGB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    /// Footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<EmbedFooter>,
    /// Image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<EmbedMedia>,
    /// Thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<EmbedMedia>,
    /// Video (cannot be set by bots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<EmbedVideo>,
    /// Provider (cannot be set by bots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<EmbedProvider>,
    /// Author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<EmbedAuthor>,
    /// Fields (max. 25).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EmbedField>,
    /// Components of a `components` link preview embed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    /// Flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedFlags>,
}

str_enum! {
    /// Embed type.
    pub enum EmbedType {
        /// Generic embed from embed attributes.
        Rich = "rich",
        /// Image.
        Image = "image",
        /// Video.
        Video = "video",
        /// Animated GIF rendered as video.
        Gifv = "gifv",
        /// Article.
        Article = "article",
        /// Link.
        Link = "link",
        /// Poll result.
        PollResult = "poll_result",
        /// Link preview built from components.
        Components = "components",
    }
}

bitflags::bitflags! {
    /// Embed flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct EmbedFlags: u64 {
        /// Media flagged as explicit.
        const CONTAINS_EXPLICIT_CONTENT = 1 << 4;
        /// Fallback for a reply to an activity card.
        const IS_CONTENT_INVENTORY_ENTRY = 1 << 5;
        /// Media flagged as gore.
        const CONTAINS_GORE_CONTENT = 1 << 6;
        /// Media flagged as self-harm.
        const CONTAINS_SELF_HARM_CONTENT = 1 << 7;
    }
}

bitflags_serde_int!(EmbedFlags, u64);

/// Embed image or thumbnail.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedMedia {
    /// Source URL (http(s) or `attachment://name`).
    pub url: Box<str>,
    /// Proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<Box<str>>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Box<str>>,
    /// Thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<Box<str>>,
    /// Placeholder version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u8>,
    /// Alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// Media flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedMediaFlags>,
}

impl EmbedMedia {
    /// Media from a URL or `attachment://name`.
    pub fn new(url: impl Into<Box<str>>) -> Self {
        Self {
            url: url.into(),
            ..Self::default()
        }
    }
}

/// Embed video (same fields as images, but the URL is optional).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedVideo {
    /// Source URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Box<str>>,
    /// Proxied URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<Box<str>>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<Box<str>>,
    /// Thumbhash placeholder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<Box<str>>,
    /// Placeholder version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder_version: Option<u8>,
    /// Alt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Box<str>>,
    /// Media flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<EmbedMediaFlags>,
}

bitflags::bitflags! {
    /// Embed media flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct EmbedMediaFlags: u64 {
        /// Animated image.
        const IS_ANIMATED = 1 << 5;
    }
}

bitflags_serde_int!(EmbedMediaFlags, u64);

/// Embed provider.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedProvider {
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Box<str>>,
    /// URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Box<str>>,
}

/// Embed author.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedAuthor {
    /// Name.
    pub name: Box<str>,
    /// URL (http(s)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Box<str>>,
    /// Icon URL (http(s) or `attachment://name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<Box<str>>,
    /// Proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<Box<str>>,
}

/// Embed footer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedFooter {
    /// Text.
    pub text: Box<str>,
    /// Icon URL (http(s) or `attachment://name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<Box<str>>,
    /// Proxied icon URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_icon_url: Option<Box<str>>,
}

/// Embed field.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct EmbedField {
    /// Name.
    pub name: Box<str>,
    /// Value.
    pub value: Box<str>,
    /// Shown inline.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inline: bool,
}
