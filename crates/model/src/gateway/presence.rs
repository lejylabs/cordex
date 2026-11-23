//! Presence types (events/gateway-events.mdx, "Update Presence", "Activity
//! Object", "Presence Update").

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{ApplicationMarker, EmojiMarker, GuildMarker};
use crate::serde_util::{bitflags_serde_int, int_enum, str_enum};
use crate::user::User;

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

int_enum! {
    /// Which activity field is shown in the member list.
    pub enum StatusDisplayType: u8 {
        /// Show `name`.
        Name = 0,
        /// Show `state`.
        State = 1,
        /// Show `details`.
        Details = 2,
    }
}

bitflags::bitflags! {
    /// Activity flags (Rich Presence).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ActivityFlags: u64 {
        /// Instanced session.
        const INSTANCE = 1 << 0;
        /// Join.
        const JOIN = 1 << 1;
        /// Spectate.
        const SPECTATE = 1 << 2;
        /// Join request.
        const JOIN_REQUEST = 1 << 3;
        /// Sync.
        const SYNC = 1 << 4;
        /// Play.
        const PLAY = 1 << 5;
        /// Party privacy: friends.
        const PARTY_PRIVACY_FRIENDS = 1 << 6;
        /// Party privacy: voice channel.
        const PARTY_PRIVACY_VOICE_CHANNEL = 1 << 7;
        /// Embedded activity.
        const EMBEDDED = 1 << 8;
    }
}

bitflags_serde_int!(ActivityFlags, u64);

/// `PRESENCE_UPDATE` payload. Fields and types are loosely validated by
/// Discord; missing values default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PresenceUpdate {
    /// User; often only `id` is present.
    #[serde(default)]
    pub user: Option<Box<User>>,
    /// Guild; omitted on some `GUILD_CREATE` partials.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Status.
    #[serde(default = "Status::default_offline")]
    pub status: Status,
    /// Current activities.
    #[serde(default)]
    pub activities: Box<[PresenceActivity]>,
    /// Per-platform status.
    #[serde(default)]
    pub client_status: ClientStatus,
}

impl Status {
    const fn default_offline() -> Self {
        Self::Offline
    }
}

/// Platform-specific status. Absent fields mean that platform is offline.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClientStatus {
    /// Desktop session.
    pub desktop: Option<Status>,
    /// Mobile session.
    pub mobile: Option<Status>,
    /// Web / bot session.
    pub web: Option<Status>,
    /// VR session.
    pub vr: Option<Status>,
}

/// Activity received over the gateway (full Rich Presence object).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PresenceActivity {
    /// Activity name.
    #[serde(default)]
    pub name: Box<str>,
    /// Activity type.
    #[serde(rename = "type", default = "ActivityType::default_playing")]
    pub kind: ActivityType,
    /// Stream URL.
    pub url: Option<Box<str>>,
    /// When the activity was added to the session, Unix milliseconds.
    #[serde(default)]
    pub created_at: u64,
    /// Start / end timestamps.
    pub timestamps: Option<ActivityTimestamps>,
    /// Game application.
    pub application_id: Option<Id<ApplicationMarker>>,
    /// Which field is shown in the member list.
    pub status_display_type: Option<StatusDisplayType>,
    /// What the player is doing.
    pub details: Option<Box<str>>,
    /// URL linked from `details`.
    pub details_url: Option<Box<str>>,
    /// Party status or custom-status text.
    pub state: Option<Box<str>>,
    /// URL linked from `state`.
    pub state_url: Option<Box<str>>,
    /// Custom-status emoji.
    pub emoji: Option<ActivityEmoji>,
    /// Party.
    pub party: Option<ActivityParty>,
    /// Images.
    pub assets: Option<ActivityAssets>,
    /// Join / spectate secrets (`Debug` redacts them).
    pub secrets: Option<ActivitySecrets>,
    /// Instanced game session.
    pub instance: Option<bool>,
    /// Activity flags.
    #[serde(default)]
    pub flags: ActivityFlags,
    /// Button labels (URLs are not sent to bots).
    #[serde(default)]
    pub buttons: Vec<Box<str>>,
}

impl ActivityType {
    const fn default_playing() -> Self {
        Self::Playing
    }
}

/// Start and end of an activity, Unix milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivityTimestamps {
    /// Start.
    pub start: Option<u64>,
    /// End.
    pub end: Option<u64>,
}

/// Emoji on a custom status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivityEmoji {
    /// Emoji name.
    pub name: Box<str>,
    /// Custom emoji ID.
    pub id: Option<Id<EmojiMarker>>,
    /// Animated.
    #[serde(default)]
    pub animated: bool,
}

/// Activity party.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivityParty {
    /// Party ID.
    pub id: Option<Box<str>>,
    /// `[current, max]`.
    pub size: Option<[u32; 2]>,
}

/// Activity images and hover text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivityAssets {
    /// Large image (asset ID or `mp:` URL).
    pub large_image: Option<Box<str>>,
    /// Large-image hover text.
    pub large_text: Option<Box<str>>,
    /// Large-image click URL.
    pub large_url: Option<Box<str>>,
    /// Small image.
    pub small_image: Option<Box<str>>,
    /// Small-image hover text.
    pub small_text: Option<Box<str>>,
    /// Small-image click URL.
    pub small_url: Option<Box<str>>,
    /// Game-invite cover image.
    pub invite_cover_image: Option<Box<str>>,
}

/// Rich Presence secrets. `Debug` never prints the values.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ActivitySecrets {
    /// Join secret.
    pub join: Option<Box<str>>,
    /// Spectate secret.
    pub spectate: Option<Box<str>>,
    /// Match secret (`match` in JSON).
    #[serde(rename = "match")]
    pub match_secret: Option<Box<str>>,
}

impl fmt::Debug for ActivitySecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActivitySecrets")
            .field("join", &self.join.as_ref().map(|_| "***"))
            .field("spectate", &self.spectate.as_ref().map(|_| "***"))
            .field("match_secret", &self.match_secret.as_ref().map(|_| "***"))
            .finish()
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

    #[test]
    fn presence_update_partial_user() {
        let json = r#"{
            "user":{"id":"1"},"guild_id":"2","status":"online",
            "activities":[],"client_status":{"desktop":"online"}
        }"#;
        let presence: PresenceUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(presence.user.as_ref().unwrap().id.get(), 1);
        assert!(presence.user.as_ref().unwrap().username.is_empty());
        assert_eq!(presence.status, Status::Online);
        assert_eq!(presence.client_status.desktop, Some(Status::Online));
    }

    #[test]
    fn rich_activity_redacts_secrets() {
        let json = r#"{
            "name":"Rocket League","type":0,
            "secrets":{"join":"secret-join","spectate":"secret-spec","match":"secret-match"}
        }"#;
        let activity: PresenceActivity = serde_json::from_str(json).unwrap();
        let debug = format!("{activity:?}");
        assert!(!debug.contains("secret-"));
        assert_eq!(
            activity.secrets.as_ref().unwrap().join.as_deref(),
            Some("secret-join")
        );
    }
}
