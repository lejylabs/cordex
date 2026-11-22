//! Debug-build warning for event filters that ask for events the intents
//! never deliver.

use cordex_model::gateway::Intents;

use crate::event_flags::EventTypeFlags;

/// Events and the intents that deliver them (any one of the listed intents is
/// enough). Events not listed need no intent.
const REQUIREMENTS: &[(EventTypeFlags, Intents)] = &[
    (
        EventTypeFlags::GUILD_CREATE
            .union(EventTypeFlags::GUILD_UPDATE)
            .union(EventTypeFlags::GUILD_DELETE)
            .union(EventTypeFlags::GUILD_ROLE_CREATE)
            .union(EventTypeFlags::GUILD_ROLE_UPDATE)
            .union(EventTypeFlags::GUILD_ROLE_DELETE)
            .union(EventTypeFlags::CHANNEL_CREATE)
            .union(EventTypeFlags::CHANNEL_UPDATE)
            .union(EventTypeFlags::CHANNEL_DELETE)
            .union(EventTypeFlags::THREAD_CREATE)
            .union(EventTypeFlags::THREAD_UPDATE)
            .union(EventTypeFlags::THREAD_DELETE)
            .union(EventTypeFlags::THREAD_LIST_SYNC)
            .union(EventTypeFlags::THREAD_MEMBER_UPDATE)
            .union(EventTypeFlags::THREAD_MEMBERS_UPDATE)
            .union(EventTypeFlags::STAGE_INSTANCE_CREATE)
            .union(EventTypeFlags::STAGE_INSTANCE_UPDATE)
            .union(EventTypeFlags::STAGE_INSTANCE_DELETE),
        Intents::GUILDS,
    ),
    (
        EventTypeFlags::CHANNEL_PINS_UPDATE,
        Intents::GUILDS.union(Intents::DIRECT_MESSAGES),
    ),
    (
        EventTypeFlags::GUILD_MEMBER_ADD
            .union(EventTypeFlags::GUILD_MEMBER_UPDATE)
            .union(EventTypeFlags::GUILD_MEMBER_REMOVE),
        Intents::GUILD_MEMBERS,
    ),
    (
        EventTypeFlags::GUILD_BAN_ADD
            .union(EventTypeFlags::GUILD_BAN_REMOVE)
            .union(EventTypeFlags::GUILD_AUDIT_LOG_ENTRY_CREATE),
        Intents::GUILD_MODERATION,
    ),
    (
        EventTypeFlags::GUILD_EMOJIS_UPDATE
            .union(EventTypeFlags::GUILD_STICKERS_UPDATE)
            .union(EventTypeFlags::GUILD_SOUNDBOARD_SOUND_CREATE)
            .union(EventTypeFlags::GUILD_SOUNDBOARD_SOUND_UPDATE)
            .union(EventTypeFlags::GUILD_SOUNDBOARD_SOUND_DELETE)
            .union(EventTypeFlags::GUILD_SOUNDBOARD_SOUNDS_UPDATE),
        Intents::GUILD_EXPRESSIONS,
    ),
    (
        EventTypeFlags::GUILD_INTEGRATIONS_UPDATE,
        Intents::GUILD_INTEGRATIONS,
    ),
    (EventTypeFlags::WEBHOOKS_UPDATE, Intents::GUILD_WEBHOOKS),
    (
        EventTypeFlags::INVITE_CREATE.union(EventTypeFlags::INVITE_DELETE),
        Intents::GUILD_INVITES,
    ),
    (
        EventTypeFlags::VOICE_STATE_UPDATE.union(EventTypeFlags::VOICE_CHANNEL_EFFECT_SEND),
        Intents::GUILD_VOICE_STATES,
    ),
    (EventTypeFlags::PRESENCE_UPDATE, Intents::GUILD_PRESENCES),
    (
        EventTypeFlags::MESSAGE_CREATE
            .union(EventTypeFlags::MESSAGE_UPDATE)
            .union(EventTypeFlags::MESSAGE_DELETE),
        Intents::GUILD_MESSAGES.union(Intents::DIRECT_MESSAGES),
    ),
    (EventTypeFlags::MESSAGE_DELETE_BULK, Intents::GUILD_MESSAGES),
    (
        EventTypeFlags::MESSAGE_REACTION_ADD
            .union(EventTypeFlags::MESSAGE_REACTION_REMOVE)
            .union(EventTypeFlags::MESSAGE_REACTION_REMOVE_ALL)
            .union(EventTypeFlags::MESSAGE_REACTION_REMOVE_EMOJI),
        Intents::GUILD_MESSAGE_REACTIONS.union(Intents::DIRECT_MESSAGE_REACTIONS),
    ),
    (
        EventTypeFlags::TYPING_START,
        Intents::GUILD_MESSAGE_TYPING.union(Intents::DIRECT_MESSAGE_TYPING),
    ),
    (
        EventTypeFlags::GUILD_SCHEDULED_EVENT_CREATE
            .union(EventTypeFlags::GUILD_SCHEDULED_EVENT_UPDATE)
            .union(EventTypeFlags::GUILD_SCHEDULED_EVENT_DELETE)
            .union(EventTypeFlags::GUILD_SCHEDULED_EVENT_USER_ADD)
            .union(EventTypeFlags::GUILD_SCHEDULED_EVENT_USER_REMOVE),
        Intents::GUILD_SCHEDULED_EVENTS,
    ),
    (
        EventTypeFlags::AUTO_MODERATION_RULE_CREATE
            .union(EventTypeFlags::AUTO_MODERATION_RULE_UPDATE)
            .union(EventTypeFlags::AUTO_MODERATION_RULE_DELETE),
        Intents::AUTO_MODERATION_CONFIGURATION,
    ),
    (
        EventTypeFlags::AUTO_MODERATION_ACTION_EXECUTION,
        Intents::AUTO_MODERATION_EXECUTION,
    ),
    (
        EventTypeFlags::MESSAGE_POLL_VOTE_ADD.union(EventTypeFlags::MESSAGE_POLL_VOTE_REMOVE),
        Intents::GUILD_MESSAGE_POLLS.union(Intents::DIRECT_MESSAGE_POLLS),
    ),
];

/// Result of comparing an explicit event filter with the intents.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct IntentReport {
    /// Requested events that no configured intent delivers.
    pub unreachable: EventTypeFlags,
    /// Intents that would deliver them (any one per event group).
    pub missing: Intents,
    /// Message events requested without `MESSAGE_CONTENT`: content,
    /// embeds, attachments and components arrive empty.
    pub content_missing: bool,
}

impl IntentReport {
    /// Nothing to report. (`EventTypeFlags::default()` is *all* events, so
    /// this type cannot derive `Default`.)
    pub(crate) const fn clean() -> Self {
        Self {
            unreachable: EventTypeFlags::empty(),
            missing: Intents::empty(),
            content_missing: false,
        }
    }
}

/// Compare `events` with `intents`. The default filter (all events) is not a
/// request for specific events, so it never reports anything.
pub(crate) fn check(intents: Intents, events: EventTypeFlags) -> IntentReport {
    let mut report = IntentReport::clean();
    if events == EventTypeFlags::all() {
        return report;
    }
    for (group, needed) in REQUIREMENTS {
        let requested = events.intersection(*group);
        if !requested.is_empty() && !intents.intersects(*needed) {
            report.unreachable |= requested;
            report.missing |= *needed;
        }
    }
    let message_events = EventTypeFlags::MESSAGE_CREATE.union(EventTypeFlags::MESSAGE_UPDATE);
    report.content_missing = events.intersects(message_events)
        && intents.intersects(Intents::GUILD_MESSAGES.union(Intents::DIRECT_MESSAGES))
        && !intents.contains(Intents::MESSAGE_CONTENT);
    report
}

/// Log the report at `warn` (called in debug builds only).
pub(crate) fn warn_missing_intents(intents: Intents, events: EventTypeFlags) {
    let report = check(intents, events);
    if !report.unreachable.is_empty() {
        tracing::warn!(
            events = ?report.unreachable,
            missing_intents = ?report.missing,
            "event filter requests events that the configured intents never deliver"
        );
    }
    if report.content_missing {
        tracing::warn!(
            "message events requested without the MESSAGE_CONTENT intent: content, embeds, \
             attachments and components will be empty (enable it in the developer portal)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_filter_reports_nothing() {
        assert_eq!(
            check(Intents::empty(), EventTypeFlags::all()),
            IntentReport::clean()
        );
    }

    #[test]
    fn member_events_without_guild_members() {
        let report = check(
            Intents::GUILDS,
            EventTypeFlags::GUILD_MEMBER_ADD | EventTypeFlags::GUILD_CREATE,
        );
        assert_eq!(report.unreachable, EventTypeFlags::GUILD_MEMBER_ADD);
        assert_eq!(report.missing, Intents::GUILD_MEMBERS);
        assert!(!report.content_missing);
    }

    #[test]
    fn any_listed_intent_is_enough() {
        let report = check(Intents::DIRECT_MESSAGES, EventTypeFlags::MESSAGE_CREATE);
        assert!(report.unreachable.is_empty());
        assert!(report.content_missing);
    }

    #[test]
    fn message_content_present() {
        let report = check(
            Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
            EventTypeFlags::MESSAGE_CREATE,
        );
        assert_eq!(report, IntentReport::clean());
    }

    #[test]
    fn events_without_intent_requirement() {
        let report = check(
            Intents::empty(),
            EventTypeFlags::READY | EventTypeFlags::INTERACTION_CREATE,
        );
        assert_eq!(report, IntentReport::clean());
    }
}
