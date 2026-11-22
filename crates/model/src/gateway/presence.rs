//! Presence types (events/gateway-events.mdx, "Update Presence", "Activity
//! Object").

use serde::Serialize;

use crate::serde_util::{int_enum, str_enum};

str_enum! {
    /// Online status.
    pub enum Status {
        /// Online.
        Online = "online",
        /// Do not disturb.
        DoNotDisturb = "dnd",
        /// AFK.
        Idle = "idle",
        /// Invisible, shown as offline.
        Invisible = "invisible",
        /// Offline.
        Offline = "offline",
    }
}

int_enum! {
    /// Activity type.
    pub enum ActivityType: u8 {
        /// "Playing {name}".
        Playing = 0,
        /// "Streaming {details}".
        Streaming = 1,
        /// "Listening to {name}".
        Listening = 2,
        /// "Watching {name}".
        Watching = 3,
        /// "{emoji} {state}".
        Custom = 4,
        /// "Competing in {name}".
        Competing = 5,
    }
}

/// An activity a bot can set. Bots may only set `name`, `state`, `type`
/// and `url` (docs, "Activity Object").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Activity {
    /// Activity name.
    pub name: String,
    /// Activity type.
    #[serde(rename = "type")]
    pub kind: ActivityType,
    /// Stream URL, validated by Discord when the type is `Streaming`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Custom status text or party status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl Activity {
    /// Activity with a name and type.
    pub fn new(kind: ActivityType, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            url: None,
            state: None,
        }
    }

    /// Custom status (`type` 4) with the text in `state`.
    pub fn custom(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            name: "Custom Status".to_owned(),
            kind: ActivityType::Custom,
            url: None,
            state: Some(text),
        }
    }

    /// Sets the stream URL.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the state text.
    #[must_use]
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }
}

/// Presence sent with opcode 3 or in Identify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct UpdatePresence {
    /// Unix milliseconds since the client went idle, `null` if not idle.
    pub since: Option<u64>,
    /// Activities.
    pub activities: Vec<Activity>,
    /// Status.
    pub status: Status,
    /// Whether the client is AFK.
    pub afk: bool,
}

impl UpdatePresence {
    /// Presence with a status and no activities.
    pub fn new(status: Status) -> Self {
        Self {
            since: None,
            activities: Vec::new(),
            status,
            afk: false,
        }
    }

    /// Adds an activity.
    #[must_use]
    pub fn activity(mut self, activity: Activity) -> Self {
        self.activities.push(activity);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_like_the_docs_example() {
        let presence = UpdatePresence {
            since: Some(91_879_201),
            activities: vec![Activity::new(
                ActivityType::Playing,
                "Save the Oxford Comma",
            )],
            status: Status::Online,
            afk: false,
        };
        assert_eq!(
            serde_json::to_string(&presence).unwrap(),
            r#"{"since":91879201,"activities":[{"name":"Save the Oxford Comma","type":0}],"status":"online","afk":false}"#
        );
    }

    #[test]
    fn since_null_and_custom_status() {
        let presence = UpdatePresence::new(Status::DoNotDisturb).activity(Activity::custom("hi"));
        assert_eq!(
            serde_json::to_string(&presence).unwrap(),
            r#"{"since":null,"activities":[{"name":"Custom Status","type":4,"state":"hi"}],"status":"dnd","afk":false}"#
        );
    }

    #[test]
    fn unknown_values() {
        assert_eq!(
            serde_json::from_str::<Status>("\"busy\"").unwrap(),
            Status::Unknown("busy".into())
        );
        assert_eq!(
            serde_json::from_str::<ActivityType>("9").unwrap(),
            ActivityType::Unknown(9)
        );
    }
}
