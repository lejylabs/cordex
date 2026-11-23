//! Embeds (resources/message.mdx, "Embed Object").

use serde::{Deserialize, Serialize};

use crate::component::Component;
use crate::serde_util::{bitflags_serde_int, str_enum};
use crate::timestamp::Timestamp;

/// Title of an embed (docs, "Embed Limits").
pub const TITLE_LIMIT: usize = 256;
/// Description of an embed.
pub const DESCRIPTION_LIMIT: usize = 4096;
/// Fields on one embed.
pub const FIELD_COUNT_LIMIT: usize = 25;
/// Field name.
pub const FIELD_NAME_LIMIT: usize = 256;
/// Field value.
pub const FIELD_VALUE_LIMIT: usize = 1024;
/// Footer text.
pub const FOOTER_LIMIT: usize = 2048;
/// Author name.
pub const AUTHOR_LIMIT: usize = 256;
/// Combined text of one embed (title, description, fields, footer, author).
pub const TOTAL_LIMIT: usize = 6000;

/// A local embed-builder check failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EmbedError {
    /// A text field is longer than Discord allows.
    #[error("embed {field} is {got} characters; max {max}")]
    TooLong {
        /// Field name.
        field: &'static str,
        /// Character count of the value.
        got: usize,
        /// Documented maximum.
        max: usize,
    },
    /// A required text (field name or value) is empty.
    #[error("embed {field} must not be empty")]
    Empty {
        /// Field name.
        field: &'static str,
    },
    /// More than [`FIELD_COUNT_LIMIT`] fields.
    #[error("an embed can have at most {FIELD_COUNT_LIMIT} fields")]
    TooManyFields,
    /// Combined text of the embed exceeds [`TOTAL_LIMIT`].
    #[error("embed text is {got} characters; max {TOTAL_LIMIT}")]
    TotalTooLong {
        /// Combined character count.
        got: usize,
    },
}

fn check_len(field: &'static str, value: &str, max: usize) -> Result<(), EmbedError> {
    let got = value.chars().count();
    if got > max {
        Err(EmbedError::TooLong { field, got, max })
    } else {
        Ok(())
    }
}

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

impl Embed {
    /// Empty rich embed for bots and webhooks.
    pub fn new() -> Self {
        Self {
            kind: Some(EmbedType::Rich),
            ..Self::default()
        }
    }

    /// Title (max. 256 characters).
    pub fn title(mut self, title: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let title = title.into();
        check_len("title", &title, TITLE_LIMIT)?;
        self.title = Some(title);
        self.check_total()?;
        Ok(self)
    }

    /// Description (max. 4096 characters).
    pub fn description(mut self, description: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let description = description.into();
        check_len("description", &description, DESCRIPTION_LIMIT)?;
        self.description = Some(description);
        self.check_total()?;
        Ok(self)
    }

    /// URL of the title.
    #[must_use]
    pub fn url(mut self, url: impl Into<Box<str>>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Timestamp shown in the footer.
    #[must_use]
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Color as `0xRRGGBB`.
    #[must_use]
    pub fn color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    /// Footer text (max. 2048 characters).
    pub fn footer(mut self, text: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let text = text.into();
        check_len("footer", &text, FOOTER_LIMIT)?;
        self.footer = Some(EmbedFooter {
            text,
            icon_url: None,
            proxy_icon_url: None,
        });
        self.check_total()?;
        Ok(self)
    }

    /// Footer with an icon URL (`http(s)` or `attachment://name`).
    pub fn footer_with_icon(
        mut self,
        text: impl Into<Box<str>>,
        icon_url: impl Into<Box<str>>,
    ) -> Result<Self, EmbedError> {
        let text = text.into();
        check_len("footer", &text, FOOTER_LIMIT)?;
        self.footer = Some(EmbedFooter {
            text,
            icon_url: Some(icon_url.into()),
            proxy_icon_url: None,
        });
        self.check_total()?;
        Ok(self)
    }

    /// Author name (max. 256 characters).
    pub fn author(mut self, name: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let name = name.into();
        check_len("author", &name, AUTHOR_LIMIT)?;
        self.author = Some(EmbedAuthor {
            name,
            url: None,
            icon_url: None,
            proxy_icon_url: None,
        });
        self.check_total()?;
        Ok(self)
    }

    /// Author with a URL and optional icon.
    pub fn author_with(
        mut self,
        name: impl Into<Box<str>>,
        url: Option<Box<str>>,
        icon_url: Option<Box<str>>,
    ) -> Result<Self, EmbedError> {
        let name = name.into();
        check_len("author", &name, AUTHOR_LIMIT)?;
        self.author = Some(EmbedAuthor {
            name,
            url,
            icon_url,
            proxy_icon_url: None,
        });
        self.check_total()?;
        Ok(self)
    }

    /// Large image (`http(s)` or `attachment://name`).
    #[must_use]
    pub fn image(mut self, url: impl Into<Box<str>>) -> Self {
        self.image = Some(EmbedMedia::new(url));
        self
    }

    /// Thumbnail (`http(s)` or `attachment://name`).
    #[must_use]
    pub fn thumbnail(mut self, url: impl Into<Box<str>>) -> Self {
        self.thumbnail = Some(EmbedMedia::new(url));
        self
    }

    /// Add a field (max. 25; name 256, value 1024 characters).
    pub fn field(
        mut self,
        name: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        inline: bool,
    ) -> Result<Self, EmbedError> {
        if self.fields.len() >= FIELD_COUNT_LIMIT {
            return Err(EmbedError::TooManyFields);
        }
        self.fields.push(EmbedField::new(name, value, inline)?);
        self.check_total()?;
        Ok(self)
    }

    /// Combined character count of title, description, fields, footer and author.
    pub fn text_len(&self) -> usize {
        let mut n = chars(self.title.as_deref()) + chars(self.description.as_deref());
        if let Some(footer) = &self.footer {
            n += footer.text.chars().count();
        }
        if let Some(author) = &self.author {
            n += author.name.chars().count();
        }
        for field in &self.fields {
            n += field.name.chars().count();
            n += field.value.chars().count();
        }
        n
    }

    fn check_total(&self) -> Result<(), EmbedError> {
        let got = self.text_len();
        if got > TOTAL_LIMIT {
            Err(EmbedError::TotalTooLong { got })
        } else {
            Ok(())
        }
    }
}

fn chars(s: Option<&str>) -> usize {
    s.map_or(0, |s| s.chars().count())
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

impl EmbedAuthor {
    /// Author with a name (max. 256 characters).
    pub fn new(name: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let name = name.into();
        check_len("author", &name, AUTHOR_LIMIT)?;
        Ok(Self {
            name,
            url: None,
            icon_url: None,
            proxy_icon_url: None,
        })
    }
}

impl EmbedFooter {
    /// Footer with text (max. 2048 characters).
    pub fn new(text: impl Into<Box<str>>) -> Result<Self, EmbedError> {
        let text = text.into();
        check_len("footer", &text, FOOTER_LIMIT)?;
        Ok(Self {
            text,
            icon_url: None,
            proxy_icon_url: None,
        })
    }
}

impl EmbedField {
    /// Field (name 256, value 1024 characters).
    pub fn new(
        name: impl Into<Box<str>>,
        value: impl Into<Box<str>>,
        inline: bool,
    ) -> Result<Self, EmbedError> {
        let name = name.into();
        let value = value.into();
        if name.trim().is_empty() {
            return Err(EmbedError::Empty {
                field: "field name",
            });
        }
        if value.trim().is_empty() {
            return Err(EmbedError::Empty {
                field: "field value",
            });
        }
        check_len("field name", &name, FIELD_NAME_LIMIT)?;
        check_len("field value", &value, FIELD_VALUE_LIMIT)?;
        Ok(Self {
            name,
            value,
            inline,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_field_texts_are_rejected() {
        assert_eq!(
            Embed::new().field("", "v", false).unwrap_err(),
            EmbedError::Empty {
                field: "field name"
            }
        );
        assert_eq!(
            Embed::new().field("n", " ", false).unwrap_err(),
            EmbedError::Empty {
                field: "field value"
            }
        );
        assert!(Embed::new().field("n", "v", true).is_ok());
    }

    #[test]
    fn builder_sets_fields_and_rejects_limits() {
        let embed = Embed::new()
            .title("Hello")
            .unwrap()
            .description("World")
            .unwrap()
            .field("A", "B", true)
            .unwrap()
            .color(0x0057_F287)
            .url("https://example.com");
        assert_eq!(embed.title.as_deref(), Some("Hello"));
        assert_eq!(embed.kind, Some(EmbedType::Rich));
        assert_eq!(embed.fields.len(), 1);
        assert_eq!(embed.text_len(), "Hello".len() + "World".len() + 2);

        assert!(matches!(
            Embed::new().title("x".repeat(TITLE_LIMIT + 1)),
            Err(EmbedError::TooLong { field: "title", .. })
        ));
        let mut many = Embed::new();
        for i in 0..FIELD_COUNT_LIMIT {
            many = many.field(format!("n{i}"), "v", false).unwrap();
        }
        assert!(matches!(
            many.field("x", "y", false),
            Err(EmbedError::TooManyFields)
        ));
    }
}
