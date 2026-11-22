//! Stage instances (resources/stage-instance.mdx).

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, ScheduledEventMarker, StageMarker};
use crate::serde_util::int_enum;

int_enum! {
    /// Who can see a stage instance.
    pub enum StagePrivacyLevel: u8 {
        /// Public discovery (deprecated).
        Public = 1,
        /// Guild members only.
        GuildOnly = 2,
    }
}

/// A live stage in a stage channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StageInstance {
    /// Stage instance ID.
    pub id: Id<StageMarker>,
    /// Guild of the stage channel.
    pub guild_id: Id<GuildMarker>,
    /// Stage channel.
    pub channel_id: Id<ChannelMarker>,
    /// Topic (1–120 characters).
    pub topic: Box<str>,
    /// Who can see the instance.
    pub privacy_level: StagePrivacyLevel,
    /// Whether Stage Discovery is disabled (deprecated).
    #[serde(default)]
    pub discoverable_disabled: bool,
    /// Linked scheduled event.
    pub guild_scheduled_event_id: Option<Id<ScheduledEventMarker>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_example() {
        let json = r#"{
            "id":"840647391636226060","guild_id":"197038439483310086",
            "channel_id":"733488538393510049","topic":"Testing Testing, 123",
            "privacy_level":1,"discoverable_disabled":false,
            "guild_scheduled_event_id":"947656305244532806"
        }"#;
        let stage: StageInstance = serde_json::from_str(json).unwrap();
        assert_eq!(stage.privacy_level, StagePrivacyLevel::Public);
        assert_eq!(&*stage.topic, "Testing Testing, 123");
        let out = serde_json::to_string(&stage).unwrap();
        let again: StageInstance = serde_json::from_str(&out).unwrap();
        assert_eq!(stage, again);
    }
}
