//! Guild templates (resources/guild-template.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{GuildMarker, UserMarker};
use crate::timestamp::Timestamp;
use crate::user::User;

/// A code that creates a guild from a snapshot of another guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildTemplate {
    /// Template code (unique ID).
    pub code: Box<str>,
    /// Name.
    pub name: Box<str>,
    /// Description.
    pub description: Option<Box<str>>,
    /// Times the template was used.
    #[serde(default)]
    pub usage_count: u64,
    /// Creator.
    pub creator_id: Id<UserMarker>,
    /// Creator user.
    pub creator: Option<User>,
    /// Created at.
    pub created_at: Timestamp,
    /// Last synced at.
    pub updated_at: Timestamp,
    /// Guild the template is based on.
    pub source_guild_id: Id<GuildMarker>,
    /// Snapshot of the source guild (partial guild: roles, channels,
    /// settings), kept as JSON.
    pub serialized_source_guild: serde_json::Value,
    /// Whether the template has unsynced changes.
    pub is_dirty: Option<bool>,
}
