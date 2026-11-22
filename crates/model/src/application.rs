//! Applications (resources/application.mdx, "Application Object").
//!
//! Only the partial object from Ready for now; the full object follows with
//! the application endpoints.

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::ApplicationMarker;
use crate::serde_util::{bitflags_serde_int, bitflags_serde_string};

/// Partial application in Ready: `id`, `flags`, `flags_new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PartialApplication {
    /// Application ID.
    pub id: Id<ApplicationMarker>,
    /// Legacy flags (number, never beyond 31 bits).
    #[serde(default)]
    pub flags: ApplicationFlags,
    /// Full flags (string; new bits beyond bit 30 only appear here).
    pub flags_new: Option<ApplicationFlagsNew>,
}

impl PartialApplication {
    /// All flags: `flags_new` if present, else the legacy `flags`.
    pub fn all_flags(&self) -> ApplicationFlags {
        self.flags_new.map_or(self.flags, |f| f.0)
    }
}

bitflags::bitflags! {
    /// Public application flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ApplicationFlags: u64 {
        /// Uses the auto moderation API (badge).
        const APPLICATION_AUTO_MODERATION_RULE_CREATE_BADGE = 1 << 6;
        /// Presence intent approved (100+ guilds).
        const GATEWAY_PRESENCE = 1 << 12;
        /// Presence intent enabled (under 100 guilds).
        const GATEWAY_PRESENCE_LIMITED = 1 << 13;
        /// Guild members intent approved (100+ guilds).
        const GATEWAY_GUILD_MEMBERS = 1 << 14;
        /// Guild members intent enabled (under 100 guilds).
        const GATEWAY_GUILD_MEMBERS_LIMITED = 1 << 15;
        /// Verification pending, guild limit reached.
        const VERIFICATION_PENDING_GUILD_LIMIT = 1 << 16;
        /// Embedded application (activity).
        const EMBEDDED = 1 << 17;
        /// Message content intent approved (100+ guilds).
        const GATEWAY_MESSAGE_CONTENT = 1 << 18;
        /// Message content intent enabled (under 100 guilds).
        const GATEWAY_MESSAGE_CONTENT_LIMITED = 1 << 19;
        /// Has at least one global application command (badge).
        const APPLICATION_COMMAND_BADGE = 1 << 23;
    }
}

bitflags_serde_int!(ApplicationFlags, u64);

/// [`ApplicationFlags`] in the string form of `flags_new`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ApplicationFlagsNew(pub ApplicationFlags);

impl ApplicationFlagsNew {
    const fn bits(self) -> u64 {
        self.0.bits()
    }

    const fn from_bits_retain(bits: u64) -> Self {
        Self(ApplicationFlags::from_bits_retain(bits))
    }
}

bitflags_serde_string!(ApplicationFlagsNew);
