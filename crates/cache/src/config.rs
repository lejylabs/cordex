//! What the cache stores. Everything is off by default.

use std::fmt;

bitflags::bitflags! {
    /// Resources the cache can store; each one is switched on by itself.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ResourceType: u32 {
        /// Guilds (without roles, emojis, stickers: those are own resources).
        const GUILDS = 1 << 0;
        /// Guild channels and DM channels (not threads).
        const CHANNELS = 1 << 1;
        /// Threads.
        const THREADS = 1 << 2;
        /// Users of cached members, plus the current user. Stored once and
        /// shared by all guilds; dropped when the last member entry goes.
        /// Without [`Self::MEMBERS`] only the current user is kept.
        const USERS = 1 << 3;
        /// Guild members (without their user object; see [`Self::USERS`]).
        const MEMBERS = 1 << 4;
        /// Roles.
        const ROLES = 1 << 5;
        /// Custom emojis.
        const EMOJIS = 1 << 6;
        /// Guild stickers.
        const STICKERS = 1 << 7;
        /// Guild soundboard sounds.
        const SOUNDBOARD_SOUNDS = 1 << 8;
        /// Voice states.
        const VOICE_STATES = 1 << 9;
        /// Presences (needs the `GUILD_PRESENCES` intent).
        const PRESENCES = 1 << 10;
        /// Stage instances.
        const STAGE_INSTANCES = 1 << 11;
        /// Scheduled events.
        const SCHEDULED_EVENTS = 1 << 12;
        /// Messages (ring buffer per channel, see
        /// [`CacheConfig::message_cache_size`]).
        const MESSAGES = 1 << 13;
    }
}

bitflags::bitflags! {
    /// Optional data inside a resource. With a part off, the cache keeps the
    /// entry but clears that data.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct CacheParts: u32 {
        /// Role IDs of members.
        const MEMBER_ROLES = 1 << 0;
        /// Nickname, guild avatar/banner, decoration and collectibles of members.
        const MEMBER_PROFILE = 1 << 1;
        /// Avatar, banner, accent color, decoration, collectibles and
        /// primary guild of users.
        const USER_PROFILE = 1 << 2;
        /// Message text.
        const MESSAGE_CONTENT = 1 << 3;
        /// Message embeds.
        const MESSAGE_EMBEDS = 1 << 4;
        /// Message attachments.
        const MESSAGE_ATTACHMENTS = 1 << 5;
        /// Message components.
        const MESSAGE_COMPONENTS = 1 << 6;
        /// Guild feature list.
        const GUILD_FEATURES = 1 << 7;
        /// Channel permission overwrites.
        const CHANNEL_OVERWRITES = 1 << 8;
        /// Member object inside voice states.
        const VOICE_STATE_MEMBER = 1 << 9;
        /// Activities of presences.
        const PRESENCE_ACTIVITIES = 1 << 10;
    }
}

/// How a [`ResourceType`] is populated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CachePolicy {
    /// Do not cache this resource (default).
    #[default]
    Off,
    /// Cache entries only when seen in live events (messages, interactions,
    /// updates, voice states, ...); skip bulk dumps such as `GUILD_CREATE`
    /// and `GUILD_MEMBERS_CHUNK`.
    Lazy,
    /// Cache everything, including bulk dumps on `GUILD_CREATE`, `READY`,
    /// `GUILD_MEMBERS_CHUNK` and `THREAD_LIST_SYNC`.
    Eager,
}

/// Cache configuration. [`CacheConfig::default`] caches nothing.
///
/// ```
/// use cordex_cache::{CacheConfig, CacheParts, CachePolicy, ResourceType};
///
/// let config = CacheConfig::new()
///     .resources(ResourceType::GUILDS | ResourceType::CHANNELS | ResourceType::MESSAGES)
///     .lazy(ResourceType::MEMBERS | ResourceType::USERS)
///     .parts(CacheParts::MESSAGE_CONTENT)
///     .message_cache_size(50);
/// assert!(config.validate().is_empty());
/// assert_eq!(config.policy(ResourceType::MEMBERS), CachePolicy::Lazy);
/// assert!(CacheConfig::default().wanted_events().is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheConfig {
    /// Resources cached eagerly (disjoint from `lazy`).
    eager: ResourceType,
    /// Resources cached lazily on first sight (disjoint from `eager`).
    lazy: ResourceType,
    parts: CacheParts,
    message_cache_size: usize,
    max_members_per_guild: Option<usize>,
    max_presences_per_guild: Option<usize>,
}

impl CacheConfig {
    /// Empty configuration (same as `default()`): nothing is cached.
    pub fn new() -> Self {
        Self::default()
    }

    /// Preset: guilds and channels (eager), no parts.
    pub fn guilds_and_channels() -> Self {
        Self::new().resources(ResourceType::GUILDS | ResourceType::CHANNELS)
    }

    /// Switch resources on eagerly (adds to what is already on, upgrades
    /// lazy resources to eager).
    #[must_use]
    pub fn resources(mut self, resources: ResourceType) -> Self {
        self.eager |= resources;
        self.lazy.remove(resources);
        self
    }

    /// Switch resources on lazily: skip bulk dumps (`GUILD_CREATE`,
    /// `GUILD_MEMBERS_CHUNK`, `THREAD_LIST_SYNC`) and cache entries only
    /// when seen in live events. Downgrades eager resources to lazy.
    #[must_use]
    pub fn lazy(mut self, resources: ResourceType) -> Self {
        self.lazy |= resources;
        self.eager.remove(resources);
        self
    }

    /// Set the [`CachePolicy`] for one or more resources.
    #[must_use]
    pub fn with_policy(self, resources: ResourceType, policy: CachePolicy) -> Self {
        match policy {
            CachePolicy::Off => self.without_resources(resources),
            CachePolicy::Lazy => self.lazy(resources),
            CachePolicy::Eager => self.resources(resources),
        }
    }

    /// Switch resources off (both eager and lazy).
    #[must_use]
    pub fn without_resources(mut self, resources: ResourceType) -> Self {
        self.eager.remove(resources);
        self.lazy.remove(resources);
        self
    }

    /// Switch parts on (adds to what is already on).
    #[must_use]
    pub fn parts(mut self, parts: CacheParts) -> Self {
        self.parts |= parts;
        self
    }

    /// Switch parts off.
    #[must_use]
    pub fn without_parts(mut self, parts: CacheParts) -> Self {
        self.parts.remove(parts);
        self
    }

    /// Messages kept per channel (default `0`: with [`ResourceType::MESSAGES`]
    /// on, nothing is stored until this is set).
    #[must_use]
    pub fn message_cache_size(mut self, size: usize) -> Self {
        self.message_cache_size = size;
        self
    }

    /// Max. members per guild (default `None`: no cap). New members beyond
    /// the cap are not stored; existing ones are still updated.
    #[must_use]
    pub fn max_members_per_guild(mut self, max: Option<usize>) -> Self {
        self.max_members_per_guild = max;
        self
    }

    /// Max. presences per guild (default `None`: no cap).
    #[must_use]
    pub fn max_presences_per_guild(mut self, max: Option<usize>) -> Self {
        self.max_presences_per_guild = max;
        self
    }

    /// All enabled resources (eager and lazy combined).
    pub fn enabled_resources(&self) -> ResourceType {
        self.eager | self.lazy
    }

    /// Resources with [`CachePolicy::Eager`].
    pub fn eager_resources(&self) -> ResourceType {
        self.eager
    }

    /// Resources with [`CachePolicy::Lazy`].
    pub fn lazy_resources(&self) -> ResourceType {
        self.lazy
    }

    /// Enabled parts.
    pub fn enabled_parts(&self) -> CacheParts {
        self.parts
    }

    /// Policy of a single `resource`. For a combined bitmask, returns
    /// [`CachePolicy::Eager`] if all bits are eager, [`CachePolicy::Lazy`]
    /// if all bits are lazy, else [`CachePolicy::Off`].
    pub fn policy(&self, resource: ResourceType) -> CachePolicy {
        if resource.is_empty() {
            CachePolicy::Off
        } else if self.eager.contains(resource) {
            CachePolicy::Eager
        } else if self.lazy.contains(resource) {
            CachePolicy::Lazy
        } else {
            CachePolicy::Off
        }
    }

    /// Whether `resource` is enabled (either [`CachePolicy::Eager`] or
    /// [`CachePolicy::Lazy`]).
    pub fn has(&self, resource: ResourceType) -> bool {
        self.enabled_resources().contains(resource)
    }

    /// Whether `resource` is enabled with [`CachePolicy::Eager`].
    pub fn is_eager(&self, resource: ResourceType) -> bool {
        self.eager.contains(resource)
    }

    /// Whether `resource` is enabled with [`CachePolicy::Lazy`].
    pub fn is_lazy(&self, resource: ResourceType) -> bool {
        self.lazy.contains(resource)
    }

    /// Whether `part` is on.
    pub fn has_part(&self, part: CacheParts) -> bool {
        self.parts.contains(part)
    }

    /// Messages kept per channel.
    pub fn messages_per_channel(&self) -> usize {
        self.message_cache_size
    }

    /// Member cap per guild.
    pub fn member_cap(&self) -> Option<usize> {
        self.max_members_per_guild
    }

    /// Presence cap per guild.
    pub fn presence_cap(&self) -> Option<usize> {
        self.max_presences_per_guild
    }

    /// Settings that will not do what they look like. Nothing is switched on
    /// automatically; this only reports.
    pub fn validate(&self) -> Vec<ConfigWarning> {
        use CacheParts as P;
        use ResourceType as R;
        let mut out = Vec::new();
        let enabled = self.enabled_resources();
        if self.has(R::MEMBERS) && !self.has(R::USERS) {
            out.push(ConfigWarning::MembersWithoutUsers);
        }
        if self.has(R::MESSAGES) && self.message_cache_size == 0 {
            out.push(ConfigWarning::MessagesWithoutSize);
        }
        let needs = [
            (P::MEMBER_ROLES | P::MEMBER_PROFILE, R::MEMBERS),
            (P::USER_PROFILE, R::USERS | R::MESSAGES),
            (
                P::MESSAGE_CONTENT
                    | P::MESSAGE_EMBEDS
                    | P::MESSAGE_ATTACHMENTS
                    | P::MESSAGE_COMPONENTS,
                R::MESSAGES,
            ),
            (P::GUILD_FEATURES, R::GUILDS),
            (P::CHANNEL_OVERWRITES, R::CHANNELS | R::THREADS),
            (P::VOICE_STATE_MEMBER, R::VOICE_STATES),
            (P::PRESENCE_ACTIVITIES, R::PRESENCES),
        ];
        for (parts, resources) in needs {
            let on = self.parts & parts;
            if !on.is_empty() && !enabled.intersects(resources) {
                out.push(ConfigWarning::PartWithoutResource {
                    parts: on,
                    needs: resources,
                });
            }
        }
        out
    }

    /// Dispatch names the cache reads with this configuration. Feed them to
    /// the gateway event filter (`EventTypeFlags` collects from names) so
    /// other events are not parsed for the cache.
    pub fn wanted_events(&self) -> Vec<&'static str> {
        use ResourceType as R;
        if self.enabled_resources().is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut push = |name: &'static str| {
            if !out.contains(&name) {
                out.push(name);
            }
        };
        if self.has(R::USERS) || self.is_eager(R::GUILDS) {
            push("READY");
        }
        // Only eager resources read the bulk `GUILD_CREATE` payload.
        if !self.eager.is_empty() {
            push("GUILD_CREATE");
        }
        // Leaving a guild always cleans up every guild-scoped store.
        push("GUILD_DELETE");
        for (resource, names) in EVENTS_BY_RESOURCE {
            if self.has(resource) {
                for name in names {
                    push(name);
                }
            }
        }
        for (resource, names) in EAGER_EVENTS_BY_RESOURCE {
            if self.is_eager(resource) {
                for name in names {
                    push(name);
                }
            }
        }
        if self.has(R::MESSAGES) {
            // Channel and thread deletes drop their messages.
            push("CHANNEL_DELETE");
            push("THREAD_DELETE");
        }
        out
    }
}

/// Live dispatch names each resource reads (both eager and lazy).
const EVENTS_BY_RESOURCE: [(ResourceType, &[&str]); 14] = {
    use ResourceType as R;
    [
        (R::GUILDS, &["GUILD_UPDATE"]),
        (
            R::CHANNELS,
            &["CHANNEL_CREATE", "CHANNEL_UPDATE", "CHANNEL_DELETE"],
        ),
        (
            R::THREADS,
            &["THREAD_CREATE", "THREAD_UPDATE", "THREAD_DELETE"],
        ),
        (
            R::USERS,
            &[
                "USER_UPDATE",
                "GUILD_MEMBER_ADD",
                "GUILD_MEMBER_UPDATE",
                "INTERACTION_CREATE",
                "MESSAGE_CREATE",
                "VOICE_STATE_UPDATE",
            ],
        ),
        (
            R::MEMBERS,
            &[
                "GUILD_MEMBER_ADD",
                "GUILD_MEMBER_UPDATE",
                "GUILD_MEMBER_REMOVE",
                "INTERACTION_CREATE",
                "MESSAGE_CREATE",
                "VOICE_STATE_UPDATE",
            ],
        ),
        (
            R::ROLES,
            &[
                "GUILD_ROLE_CREATE",
                "GUILD_ROLE_UPDATE",
                "GUILD_ROLE_DELETE",
            ],
        ),
        (R::EMOJIS, &["GUILD_EMOJIS_UPDATE"]),
        (R::STICKERS, &["GUILD_STICKERS_UPDATE"]),
        (
            R::SOUNDBOARD_SOUNDS,
            &[
                "GUILD_SOUNDBOARD_SOUND_CREATE",
                "GUILD_SOUNDBOARD_SOUND_UPDATE",
                "GUILD_SOUNDBOARD_SOUND_DELETE",
                "GUILD_SOUNDBOARD_SOUNDS_UPDATE",
            ],
        ),
        (R::VOICE_STATES, &["VOICE_STATE_UPDATE"]),
        (R::PRESENCES, &["PRESENCE_UPDATE"]),
        (
            R::STAGE_INSTANCES,
            &[
                "STAGE_INSTANCE_CREATE",
                "STAGE_INSTANCE_UPDATE",
                "STAGE_INSTANCE_DELETE",
            ],
        ),
        (
            R::SCHEDULED_EVENTS,
            &[
                "GUILD_SCHEDULED_EVENT_CREATE",
                "GUILD_SCHEDULED_EVENT_UPDATE",
                "GUILD_SCHEDULED_EVENT_DELETE",
            ],
        ),
        (
            R::MESSAGES,
            &[
                "MESSAGE_CREATE",
                "MESSAGE_UPDATE",
                "MESSAGE_DELETE",
                "MESSAGE_DELETE_BULK",
            ],
        ),
    ]
};

/// Bulk dispatch names read only when the resource is [`CachePolicy::Eager`].
const EAGER_EVENTS_BY_RESOURCE: [(ResourceType, &[&str]); 4] = {
    use ResourceType as R;
    [
        (R::THREADS, &["THREAD_LIST_SYNC"]),
        (R::USERS, &["GUILD_MEMBERS_CHUNK"]),
        (R::MEMBERS, &["GUILD_MEMBERS_CHUNK"]),
        (R::SOUNDBOARD_SOUNDS, &["SOUNDBOARD_SOUNDS"]),
    ]
};

/// A configuration that will not do what it looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigWarning {
    /// Members are stored without their user objects.
    MembersWithoutUsers,
    /// Messages are on but [`CacheConfig::message_cache_size`] is 0.
    MessagesWithoutSize,
    /// Parts are on but none of the resources they belong to.
    PartWithoutResource {
        /// Parts that have no effect.
        parts: CacheParts,
        /// Resources of which at least one is needed.
        needs: ResourceType,
    },
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MembersWithoutUsers => {
                f.write_str("MEMBERS is on without USERS: members are stored without user objects")
            }
            Self::MessagesWithoutSize => {
                f.write_str("MESSAGES is on but message_cache_size is 0: no messages are stored")
            }
            Self::PartWithoutResource { parts, needs } => {
                write!(f, "parts {parts:?} have no effect without {needs:?}")
            }
        }
    }
}
