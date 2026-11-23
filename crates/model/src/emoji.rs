//! Emojis (resources/emoji.mdx, "Emoji Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{EmojiMarker, RoleMarker};
use crate::user::User;

/// A custom or unicode emoji.
///
/// Unicode emojis have no `id` and the character in `name`. In reaction
/// events `name` can be `None` when the custom emoji was deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Emoji {
    /// Emoji ID; `None` for unicode emojis.
    pub id: Option<Id<EmojiMarker>>,
    /// Emoji name or unicode character.
    pub name: Option<Box<str>>,
    /// Roles allowed to use this emoji.
    #[serde(default)]
    pub roles: Vec<Id<RoleMarker>>,
    /// User that created this emoji.
    pub user: Option<Box<User>>,
    /// Whether the emoji must be wrapped in colons.
    pub require_colons: Option<bool>,
    /// Whether the emoji is managed by an integration.
    pub managed: Option<bool>,
    /// Whether the emoji is animated.
    #[serde(default)]
    pub animated: bool,
    /// Whether the emoji can be used (false when boosts were lost).
    pub available: Option<bool>,
}

impl Emoji {
    /// Whether this is a unicode emoji.
    pub const fn is_unicode(&self) -> bool {
        self.id.is_none()
    }

    /// Text to use the emoji in a message: `<:name:id>`, `<a:name:id>` or
    /// the unicode character.
    pub fn mention(&self) -> String {
        let name = self.name.as_deref().unwrap_or("_");
        match self.id {
            Some(id) if self.animated => format!("<a:{name}:{id}>"),
            Some(id) => format!("<:{name}:{id}>"),
            None => name.to_owned(),
        }
    }

    /// CDN URL of a custom emoji (`None` for unicode).
    pub fn url(&self) -> Option<crate::cdn::CdnUrl> {
        self.id.map(|id| crate::cdn::emoji(id, self.animated))
    }
}
