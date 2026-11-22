//! Which dispatch events are deserialized fully (PLAN.md §10.3).

bitflags::bitflags! {
    /// Bitmask of typed gateway event names. Unset events are skipped after
    /// the envelope (`t` and `s`) is read. [`EventTypeFlags::UNKNOWN`] keeps
    /// names without a typed variant.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EventTypeFlags: u128 {
        /// `READY`.
        const READY = 1 << 0;
        /// `RESUMED`.
        const RESUMED = 1 << 1;
        /// `GUILD_CREATE`.
        const GUILD_CREATE = 1 << 2;
        /// `GUILD_UPDATE`.
        const GUILD_UPDATE = 1 << 3;
        /// `GUILD_DELETE`.
        const GUILD_DELETE = 1 << 4;
        /// `GUILD_BAN_ADD`.
        const GUILD_BAN_ADD = 1 << 5;
        /// `GUILD_BAN_REMOVE`.
        const GUILD_BAN_REMOVE = 1 << 6;
        /// `GUILD_EMOJIS_UPDATE`.
        const GUILD_EMOJIS_UPDATE = 1 << 7;
        /// `GUILD_STICKERS_UPDATE`.
        const GUILD_STICKERS_UPDATE = 1 << 8;
        /// `GUILD_INTEGRATIONS_UPDATE`.
        const GUILD_INTEGRATIONS_UPDATE = 1 << 9;
        /// `GUILD_MEMBER_ADD`.
        const GUILD_MEMBER_ADD = 1 << 10;
        /// `GUILD_MEMBER_REMOVE`.
        const GUILD_MEMBER_REMOVE = 1 << 11;
        /// `GUILD_MEMBER_UPDATE`.
        const GUILD_MEMBER_UPDATE = 1 << 12;
        /// `GUILD_MEMBERS_CHUNK`.
        const GUILD_MEMBERS_CHUNK = 1 << 13;
        /// `GUILD_ROLE_CREATE`.
        const GUILD_ROLE_CREATE = 1 << 14;
        /// `GUILD_ROLE_UPDATE`.
        const GUILD_ROLE_UPDATE = 1 << 15;
        /// `GUILD_ROLE_DELETE`.
        const GUILD_ROLE_DELETE = 1 << 16;
        /// `CHANNEL_CREATE`.
        const CHANNEL_CREATE = 1 << 17;
        /// `CHANNEL_UPDATE`.
        const CHANNEL_UPDATE = 1 << 18;
        /// `CHANNEL_DELETE`.
        const CHANNEL_DELETE = 1 << 19;
        /// `CHANNEL_PINS_UPDATE`.
        const CHANNEL_PINS_UPDATE = 1 << 20;
        /// `THREAD_CREATE`.
        const THREAD_CREATE = 1 << 21;
        /// `THREAD_UPDATE`.
        const THREAD_UPDATE = 1 << 22;
        /// `THREAD_DELETE`.
        const THREAD_DELETE = 1 << 23;
        /// `THREAD_LIST_SYNC`.
        const THREAD_LIST_SYNC = 1 << 24;
        /// `THREAD_MEMBER_UPDATE`.
        const THREAD_MEMBER_UPDATE = 1 << 25;
        /// `THREAD_MEMBERS_UPDATE`.
        const THREAD_MEMBERS_UPDATE = 1 << 26;
        /// `MESSAGE_CREATE`.
        const MESSAGE_CREATE = 1 << 27;
        /// `MESSAGE_UPDATE`.
        const MESSAGE_UPDATE = 1 << 28;
        /// `MESSAGE_DELETE`.
        const MESSAGE_DELETE = 1 << 29;
        /// `MESSAGE_DELETE_BULK`.
        const MESSAGE_DELETE_BULK = 1 << 30;
        /// `MESSAGE_REACTION_ADD`.
        const MESSAGE_REACTION_ADD = 1 << 31;
        /// `MESSAGE_REACTION_REMOVE`.
        const MESSAGE_REACTION_REMOVE = 1 << 32;
        /// `MESSAGE_REACTION_REMOVE_ALL`.
        const MESSAGE_REACTION_REMOVE_ALL = 1 << 33;
        /// `MESSAGE_REACTION_REMOVE_EMOJI`.
        const MESSAGE_REACTION_REMOVE_EMOJI = 1 << 34;
        /// `INTERACTION_CREATE`.
        const INTERACTION_CREATE = 1 << 35;
        /// `APPLICATION_COMMAND_PERMISSIONS_UPDATE`.
        const APPLICATION_COMMAND_PERMISSIONS_UPDATE = 1 << 36;
        /// `VOICE_STATE_UPDATE`.
        const VOICE_STATE_UPDATE = 1 << 37;
        /// `VOICE_SERVER_UPDATE`.
        const VOICE_SERVER_UPDATE = 1 << 38;
        /// `ENTITLEMENT_CREATE`.
        const ENTITLEMENT_CREATE = 1 << 39;
        /// `ENTITLEMENT_UPDATE`.
        const ENTITLEMENT_UPDATE = 1 << 40;
        /// `ENTITLEMENT_DELETE`.
        const ENTITLEMENT_DELETE = 1 << 41;
        /// `USER_UPDATE`.
        const USER_UPDATE = 1 << 42;
        /// `WEBHOOKS_UPDATE`.
        const WEBHOOKS_UPDATE = 1 << 43;
        /// `AUTO_MODERATION_RULE_CREATE`.
        const AUTO_MODERATION_RULE_CREATE = 1 << 44;
        /// `AUTO_MODERATION_RULE_UPDATE`.
        const AUTO_MODERATION_RULE_UPDATE = 1 << 45;
        /// `AUTO_MODERATION_RULE_DELETE`.
        const AUTO_MODERATION_RULE_DELETE = 1 << 46;
        /// `AUTO_MODERATION_ACTION_EXECUTION`.
        const AUTO_MODERATION_ACTION_EXECUTION = 1 << 47;
        /// `GUILD_SCHEDULED_EVENT_CREATE`.
        const GUILD_SCHEDULED_EVENT_CREATE = 1 << 48;
        /// `GUILD_SCHEDULED_EVENT_UPDATE`.
        const GUILD_SCHEDULED_EVENT_UPDATE = 1 << 49;
        /// `GUILD_SCHEDULED_EVENT_DELETE`.
        const GUILD_SCHEDULED_EVENT_DELETE = 1 << 50;
        /// `GUILD_SCHEDULED_EVENT_USER_ADD`.
        const GUILD_SCHEDULED_EVENT_USER_ADD = 1 << 51;
        /// `GUILD_SCHEDULED_EVENT_USER_REMOVE`.
        const GUILD_SCHEDULED_EVENT_USER_REMOVE = 1 << 52;
        /// `INVITE_CREATE`.
        const INVITE_CREATE = 1 << 53;
        /// `INVITE_DELETE`.
        const INVITE_DELETE = 1 << 54;
        /// `STAGE_INSTANCE_CREATE`.
        const STAGE_INSTANCE_CREATE = 1 << 55;
        /// `STAGE_INSTANCE_UPDATE`.
        const STAGE_INSTANCE_UPDATE = 1 << 56;
        /// `STAGE_INSTANCE_DELETE`.
        const STAGE_INSTANCE_DELETE = 1 << 57;
        /// `GUILD_SOUNDBOARD_SOUND_CREATE`.
        const GUILD_SOUNDBOARD_SOUND_CREATE = 1 << 58;
        /// `GUILD_SOUNDBOARD_SOUND_UPDATE`.
        const GUILD_SOUNDBOARD_SOUND_UPDATE = 1 << 59;
        /// `GUILD_SOUNDBOARD_SOUND_DELETE`.
        const GUILD_SOUNDBOARD_SOUND_DELETE = 1 << 60;
        /// `GUILD_SOUNDBOARD_SOUNDS_UPDATE`.
        const GUILD_SOUNDBOARD_SOUNDS_UPDATE = 1 << 61;
        /// `SOUNDBOARD_SOUNDS`.
        const SOUNDBOARD_SOUNDS = 1 << 62;
        /// `PRESENCE_UPDATE`.
        const PRESENCE_UPDATE = 1 << 63;
        /// `TYPING_START`.
        const TYPING_START = 1 << 64;
        /// `GUILD_AUDIT_LOG_ENTRY_CREATE`.
        const GUILD_AUDIT_LOG_ENTRY_CREATE = 1 << 65;
        /// `MESSAGE_POLL_VOTE_ADD`.
        const MESSAGE_POLL_VOTE_ADD = 1 << 66;
        /// `MESSAGE_POLL_VOTE_REMOVE`.
        const MESSAGE_POLL_VOTE_REMOVE = 1 << 67;
        /// `VOICE_CHANNEL_EFFECT_SEND`.
        const VOICE_CHANNEL_EFFECT_SEND = 1 << 68;
        /// Events without a typed variant (`Event::Unknown`).
        const UNKNOWN = 1 << 69;
    }
}

impl EventTypeFlags {
    /// Flag for a dispatch name, or [`Self::UNKNOWN`] when the name has no
    /// typed variant.
    #[must_use]
    pub fn for_name(name: &str) -> Self {
        match name {
            "READY" => Self::READY,
            "RESUMED" => Self::RESUMED,
            "GUILD_CREATE" => Self::GUILD_CREATE,
            "GUILD_UPDATE" => Self::GUILD_UPDATE,
            "GUILD_DELETE" => Self::GUILD_DELETE,
            "GUILD_BAN_ADD" => Self::GUILD_BAN_ADD,
            "GUILD_BAN_REMOVE" => Self::GUILD_BAN_REMOVE,
            "GUILD_EMOJIS_UPDATE" => Self::GUILD_EMOJIS_UPDATE,
            "GUILD_STICKERS_UPDATE" => Self::GUILD_STICKERS_UPDATE,
            "GUILD_INTEGRATIONS_UPDATE" => Self::GUILD_INTEGRATIONS_UPDATE,
            "GUILD_MEMBER_ADD" => Self::GUILD_MEMBER_ADD,
            "GUILD_MEMBER_REMOVE" => Self::GUILD_MEMBER_REMOVE,
            "GUILD_MEMBER_UPDATE" => Self::GUILD_MEMBER_UPDATE,
            "GUILD_MEMBERS_CHUNK" => Self::GUILD_MEMBERS_CHUNK,
            "GUILD_ROLE_CREATE" => Self::GUILD_ROLE_CREATE,
            "GUILD_ROLE_UPDATE" => Self::GUILD_ROLE_UPDATE,
            "GUILD_ROLE_DELETE" => Self::GUILD_ROLE_DELETE,
            "CHANNEL_CREATE" => Self::CHANNEL_CREATE,
            "CHANNEL_UPDATE" => Self::CHANNEL_UPDATE,
            "CHANNEL_DELETE" => Self::CHANNEL_DELETE,
            "CHANNEL_PINS_UPDATE" => Self::CHANNEL_PINS_UPDATE,
            "THREAD_CREATE" => Self::THREAD_CREATE,
            "THREAD_UPDATE" => Self::THREAD_UPDATE,
            "THREAD_DELETE" => Self::THREAD_DELETE,
            "THREAD_LIST_SYNC" => Self::THREAD_LIST_SYNC,
            "THREAD_MEMBER_UPDATE" => Self::THREAD_MEMBER_UPDATE,
            "THREAD_MEMBERS_UPDATE" => Self::THREAD_MEMBERS_UPDATE,
            "MESSAGE_CREATE" => Self::MESSAGE_CREATE,
            "MESSAGE_UPDATE" => Self::MESSAGE_UPDATE,
            "MESSAGE_DELETE" => Self::MESSAGE_DELETE,
            "MESSAGE_DELETE_BULK" => Self::MESSAGE_DELETE_BULK,
            "MESSAGE_REACTION_ADD" => Self::MESSAGE_REACTION_ADD,
            "MESSAGE_REACTION_REMOVE" => Self::MESSAGE_REACTION_REMOVE,
            "MESSAGE_REACTION_REMOVE_ALL" => Self::MESSAGE_REACTION_REMOVE_ALL,
            "MESSAGE_REACTION_REMOVE_EMOJI" => Self::MESSAGE_REACTION_REMOVE_EMOJI,
            "INTERACTION_CREATE" => Self::INTERACTION_CREATE,
            "APPLICATION_COMMAND_PERMISSIONS_UPDATE" => {
                Self::APPLICATION_COMMAND_PERMISSIONS_UPDATE
            }
            "VOICE_STATE_UPDATE" => Self::VOICE_STATE_UPDATE,
            "VOICE_SERVER_UPDATE" => Self::VOICE_SERVER_UPDATE,
            "ENTITLEMENT_CREATE" => Self::ENTITLEMENT_CREATE,
            "ENTITLEMENT_UPDATE" => Self::ENTITLEMENT_UPDATE,
            "ENTITLEMENT_DELETE" => Self::ENTITLEMENT_DELETE,
            "USER_UPDATE" => Self::USER_UPDATE,
            "WEBHOOKS_UPDATE" => Self::WEBHOOKS_UPDATE,
            "AUTO_MODERATION_RULE_CREATE" => Self::AUTO_MODERATION_RULE_CREATE,
            "AUTO_MODERATION_RULE_UPDATE" => Self::AUTO_MODERATION_RULE_UPDATE,
            "AUTO_MODERATION_RULE_DELETE" => Self::AUTO_MODERATION_RULE_DELETE,
            "AUTO_MODERATION_ACTION_EXECUTION" => Self::AUTO_MODERATION_ACTION_EXECUTION,
            "GUILD_SCHEDULED_EVENT_CREATE" => Self::GUILD_SCHEDULED_EVENT_CREATE,
            "GUILD_SCHEDULED_EVENT_UPDATE" => Self::GUILD_SCHEDULED_EVENT_UPDATE,
            "GUILD_SCHEDULED_EVENT_DELETE" => Self::GUILD_SCHEDULED_EVENT_DELETE,
            "GUILD_SCHEDULED_EVENT_USER_ADD" => Self::GUILD_SCHEDULED_EVENT_USER_ADD,
            "GUILD_SCHEDULED_EVENT_USER_REMOVE" => Self::GUILD_SCHEDULED_EVENT_USER_REMOVE,
            "INVITE_CREATE" => Self::INVITE_CREATE,
            "INVITE_DELETE" => Self::INVITE_DELETE,
            "STAGE_INSTANCE_CREATE" => Self::STAGE_INSTANCE_CREATE,
            "STAGE_INSTANCE_UPDATE" => Self::STAGE_INSTANCE_UPDATE,
            "STAGE_INSTANCE_DELETE" => Self::STAGE_INSTANCE_DELETE,
            "GUILD_SOUNDBOARD_SOUND_CREATE" => Self::GUILD_SOUNDBOARD_SOUND_CREATE,
            "GUILD_SOUNDBOARD_SOUND_UPDATE" => Self::GUILD_SOUNDBOARD_SOUND_UPDATE,
            "GUILD_SOUNDBOARD_SOUND_DELETE" => Self::GUILD_SOUNDBOARD_SOUND_DELETE,
            "GUILD_SOUNDBOARD_SOUNDS_UPDATE" => Self::GUILD_SOUNDBOARD_SOUNDS_UPDATE,
            "SOUNDBOARD_SOUNDS" => Self::SOUNDBOARD_SOUNDS,
            "PRESENCE_UPDATE" => Self::PRESENCE_UPDATE,
            "TYPING_START" => Self::TYPING_START,
            "GUILD_AUDIT_LOG_ENTRY_CREATE" => Self::GUILD_AUDIT_LOG_ENTRY_CREATE,
            "MESSAGE_POLL_VOTE_ADD" => Self::MESSAGE_POLL_VOTE_ADD,
            "MESSAGE_POLL_VOTE_REMOVE" => Self::MESSAGE_POLL_VOTE_REMOVE,
            "VOICE_CHANNEL_EFFECT_SEND" => Self::VOICE_CHANNEL_EFFECT_SEND,
            _ => Self::UNKNOWN,
        }
    }

    /// Whether a dispatch with this name should be deserialized and delivered.
    #[must_use]
    pub fn wants(self, name: &str) -> bool {
        self.contains(Self::for_name(name))
    }
}

impl Default for EventTypeFlags {
    fn default() -> Self {
        Self::all()
    }
}
