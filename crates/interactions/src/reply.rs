//! Message content for interaction responses and edits.

use cordex_model::component::Component;
use cordex_model::interaction::InteractionCallbackData;
use cordex_model::message::{AllowedMentions, Embed, MessageFlags};

/// Message sent as an interaction response, edit or follow-up.
///
/// ```
/// use cordex_interactions::Reply;
/// let reply = Reply::new("Saved!").ephemeral();
/// # let _ = reply;
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reply {
    pub(crate) content: Option<String>,
    pub(crate) embeds: Vec<Embed>,
    pub(crate) components: Option<Vec<Component>>,
    pub(crate) allowed_mentions: Option<AllowedMentions>,
    pub(crate) flags: MessageFlags,
    pub(crate) tts: bool,
}

impl Reply {
    /// Reply with text content (max. 2000 characters, checked on send).
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            ..Self::default()
        }
    }

    /// Reply with one embed and no text.
    pub fn embed(embed: Embed) -> Self {
        Self {
            embeds: vec![embed],
            ..Self::default()
        }
    }

    /// Set or replace the text content.
    #[must_use]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Add an embed (max. 10).
    #[must_use]
    pub fn add_embed(mut self, embed: Embed) -> Self {
        self.embeds.push(embed);
        self
    }

    /// Message components (buttons, selects, …).
    #[must_use]
    pub fn components(mut self, components: Vec<Component>) -> Self {
        self.components = Some(components);
        self
    }

    /// Allowed mentions.
    #[must_use]
    pub fn allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
        self.allowed_mentions = Some(mentions);
        self
    }

    /// Only the invoking user sees the message (initial response and
    /// follow-ups; an edit cannot change it).
    #[must_use]
    pub fn ephemeral(mut self) -> Self {
        self.flags |= MessageFlags::EPHEMERAL;
        self
    }

    /// Extra message flags (`SUPPRESS_EMBEDS`, …).
    #[must_use]
    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.flags |= flags;
        self
    }

    /// Text-to-speech (initial response only).
    #[must_use]
    pub fn tts(mut self) -> Self {
        self.tts = true;
        self
    }

    pub(crate) fn into_callback_data(self) -> InteractionCallbackData {
        let mut data = InteractionCallbackData::default();
        data.content = self.content.map(String::into_boxed_str);
        data.embeds = self.embeds;
        data.components = self.components.unwrap_or_default();
        data.allowed_mentions = self.allowed_mentions;
        data.flags = self.flags;
        data.tts = self.tts.then_some(true);
        data
    }
}

impl From<&str> for Reply {
    fn from(content: &str) -> Self {
        Self::new(content)
    }
}

impl From<String> for Reply {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl From<Embed> for Reply {
    fn from(embed: Embed) -> Self {
        Self::embed(embed)
    }
}
