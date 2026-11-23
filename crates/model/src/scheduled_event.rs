//! Guild scheduled events (resources/guild-scheduled-event.mdx).

use serde::{Deserialize, Serialize};

use crate::guild::Member;
use crate::id::Id;
use crate::id::marker::{
    ChannelMarker, GenericMarker, GuildMarker, ScheduledEventMarker, UserMarker,
};
use crate::image_hash::ImageHash;
use crate::serde_util::int_enum;
use crate::timestamp::Timestamp;
use crate::user::User;

int_enum! {
    /// Who can see a scheduled event.
    pub enum ScheduledEventPrivacyLevel: u8 {
        /// Guild members only.
        GuildOnly = 2,
    }
}

int_enum! {
    /// Where the event takes place.
    pub enum ScheduledEventEntityType: u8 {
        /// Stage channel.
        StageInstance = 1,
        /// Voice channel.
        Voice = 2,
        /// External location.
        External = 3,
    }
}

int_enum! {
    /// Lifecycle of a scheduled event.
    pub enum ScheduledEventStatus: u8 {
        /// Not started.
        Scheduled = 1,
        /// Currently running.
        Active = 2,
        /// Finished (terminal).
        Completed = 3,
        /// Cancelled (terminal).
        Canceled = 4,
    }
}

int_enum! {
    /// How often a recurrence rule repeats (iCalendar subset).
    pub enum RecurrenceFrequency: u8 {
        /// Once a year.
        Yearly = 0,
        /// Once a month.
        Monthly = 1,
        /// Once a week.
        Weekly = 2,
        /// Once a day.
        Daily = 3,
    }
}

int_enum! {
    /// Day of the week in a recurrence rule (`0` = Monday).
    pub enum RecurrenceWeekday: u8 {
        /// Monday.
        Monday = 0,
        /// Tuesday.
        Tuesday = 1,
        /// Wednesday.
        Wednesday = 2,
        /// Thursday.
        Thursday = 3,
        /// Friday.
        Friday = 4,
        /// Saturday.
        Saturday = 5,
        /// Sunday.
        Sunday = 6,
    }
}

int_enum! {
    /// Month in a recurrence rule (`1` = January).
    pub enum RecurrenceMonth: u8 {
        /// January.
        January = 1,
        /// February.
        February = 2,
        /// March.
        March = 3,
        /// April.
        April = 4,
        /// May.
        May = 5,
        /// June.
        June = 6,
        /// July.
        July = 7,
        /// August.
        August = 8,
        /// September.
        September = 9,
        /// October.
        October = 10,
        /// November.
        November = 11,
        /// December.
        December = 12,
    }
}

/// A scheduled event in a guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScheduledEvent {
    /// Event ID.
    pub id: Id<ScheduledEventMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Host channel; `None` when [`ScheduledEventEntityType::External`].
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Creator; `None` for events created before 2021-10-25.
    pub creator_id: Option<Id<UserMarker>>,
    /// Name (1–100 characters).
    pub name: Box<str>,
    /// Description (1–1000 characters).
    pub description: Option<Box<str>>,
    /// Start time.
    pub scheduled_start_time: Timestamp,
    /// End time; required when the entity type is external.
    pub scheduled_end_time: Option<Timestamp>,
    /// Who can access the event.
    pub privacy_level: ScheduledEventPrivacyLevel,
    /// Lifecycle status.
    pub status: ScheduledEventStatus,
    /// Where the event takes place.
    pub entity_type: ScheduledEventEntityType,
    /// Associated entity (e.g. a stage instance).
    pub entity_id: Option<Id<GenericMarker>>,
    /// Extra data for the entity type (location for external events).
    pub entity_metadata: Option<ScheduledEventEntityMetadata>,
    /// Creator (omitted for events created before 2021-10-25).
    pub creator: Option<User>,
    /// Subscriber count (`with_user_count`).
    pub user_count: Option<u32>,
    /// Cover image hash.
    pub image: Option<ImageHash>,
    /// Recurrence; `None` for a one-off event.
    #[serde(default)]
    pub recurrence_rule: Option<RecurrenceRule>,
}

impl ScheduledEvent {
    /// CDN URL of the cover image, if set.
    pub fn cover_url(&self) -> Option<crate::cdn::CdnUrl> {
        self.image
            .as_ref()
            .map(|h| crate::cdn::scheduled_event_cover(self.id, h))
    }
}

/// Extra data for an event entity. `location` is required for external events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScheduledEventEntityMetadata {
    /// Location (1–100 characters).
    pub location: Option<Box<str>>,
}

impl ScheduledEventEntityMetadata {
    /// External location (1–100 characters).
    pub fn location(location: impl Into<Box<str>>) -> Self {
        Self {
            location: Some(location.into()),
        }
    }
}

/// A user subscribed to a scheduled event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ScheduledEventUser {
    /// Event.
    pub guild_scheduled_event_id: Id<ScheduledEventMarker>,
    /// Subscriber.
    pub user: User,
    /// Member data when requested and the user is in the guild.
    pub member: Option<Member>,
}

/// How often the event repeats (iCalendar / dateutil subset).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RecurrenceRule {
    /// Start of the recurrence interval.
    pub start: Timestamp,
    /// End of the recurrence interval (API-set; cannot be set by apps).
    pub end: Option<Timestamp>,
    /// How often the event occurs.
    pub frequency: RecurrenceFrequency,
    /// Spacing between occurrences (`2` + weekly = every other week).
    pub interval: u32,
    /// Days of the week to recur on.
    pub by_weekday: Option<Vec<RecurrenceWeekday>>,
    /// Nth weekday of the month (monthly events).
    pub by_n_weekday: Option<Vec<RecurrenceNWeekday>>,
    /// Months to recur on (yearly events).
    pub by_month: Option<Vec<RecurrenceMonth>>,
    /// Dates within a month to recur on.
    pub by_month_day: Option<Vec<i32>>,
    /// Days of the year (API-set; cannot be set by apps).
    pub by_year_day: Option<Vec<i32>>,
    /// Maximum number of occurrences (API-set; cannot be set by apps).
    pub count: Option<u32>,
}

/// Nth occurrence of a weekday in a month (`n` is 1–5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RecurrenceNWeekday {
    /// Which week of the month (1–5).
    pub n: u8,
    /// Day of that week.
    pub day: RecurrenceWeekday,
}

/// `GUILD_SCHEDULED_EVENT_USER_ADD` / `GUILD_SCHEDULED_EVENT_USER_REMOVE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GuildScheduledEventUserAdd {
    /// Event.
    pub guild_scheduled_event_id: Id<ScheduledEventMarker>,
    /// User.
    pub user_id: Id<UserMarker>,
    /// Guild.
    pub guild_id: Id<GuildMarker>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_event_round_trip() {
        let json = r#"{
            "id":"1","guild_id":"2","channel_id":null,"creator_id":"3",
            "name":"LAN","description":null,
            "scheduled_start_time":"2022-01-01T00:00:00+00:00",
            "scheduled_end_time":"2022-01-01T04:00:00+00:00",
            "privacy_level":2,"status":1,"entity_type":3,"entity_id":null,
            "entity_metadata":{"location":"Berlin"},
            "recurrence_rule":null
        }"#;
        let event: ScheduledEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.entity_type, ScheduledEventEntityType::External);
        assert_eq!(event.status, ScheduledEventStatus::Scheduled);
        assert_eq!(
            event.entity_metadata.as_ref().unwrap().location.as_deref(),
            Some("Berlin")
        );
        let out = serde_json::to_string(&event).unwrap();
        let again: ScheduledEvent = serde_json::from_str(&out).unwrap();
        assert_eq!(event, again);
    }

    #[test]
    fn recurrence_every_other_wednesday() {
        let json = r#"{
            "start":"2022-01-01T00:00:00+00:00","end":null,
            "frequency":2,"interval":2,"by_weekday":[2],
            "by_n_weekday":null,"by_month":null,"by_month_day":null,
            "by_year_day":null,"count":null
        }"#;
        let rule: RecurrenceRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.frequency, RecurrenceFrequency::Weekly);
        assert_eq!(rule.interval, 2);
        assert_eq!(
            rule.by_weekday.as_deref(),
            Some(&[RecurrenceWeekday::Wednesday][..])
        );
    }
}
