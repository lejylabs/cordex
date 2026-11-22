//! Dispatch events (opcode 0).
//!
//! Typed variants are added per phase as their models exist; every other
//! event arrives as [`Event::Unknown`] with its raw JSON, so a new Discord
//! event never breaks a shard (PLAN.md D9, §8.3).

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::application::PartialApplication;
use crate::channel::Channel;
use crate::guild::{Guild, UnavailableGuild};
use crate::user::User;

/// A dispatched gateway event.
///
/// Large payloads are boxed so the enum stays small (events travel through
/// channels).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// `READY`: the session is established.
    Ready(Box<Ready>),
    /// `RESUMED`: a resume finished; missed events were replayed before it.
    Resumed,
    /// `GUILD_UPDATE`.
    GuildUpdate(Box<Guild>),
    /// `GUILD_DELETE`: guild unavailable (`unavailable: true`) or the bot
    /// was removed (`unavailable: false`).
    GuildDelete(UnavailableGuild),
    /// `CHANNEL_CREATE`.
    ChannelCreate(Box<Channel>),
    /// `CHANNEL_UPDATE`.
    ChannelUpdate(Box<Channel>),
    /// `CHANNEL_DELETE`.
    ChannelDelete(Box<Channel>),
    /// An event without a typed variant (yet).
    Unknown(UnknownEvent),
}

/// `READY` payload (events/gateway-events.mdx, "Ready").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ready {
    /// API version.
    pub v: u8,
    /// The bot user.
    pub user: User,
    /// Guilds of this shard, all unavailable until their `GUILD_CREATE`.
    pub guilds: Vec<UnavailableGuild>,
    /// Session ID for resuming.
    pub session_id: Box<str>,
    /// Gateway URL for resuming.
    pub resume_gateway_url: Box<str>,
    /// `[shard_id, num_shards]`.
    pub shard: Option<[u32; 2]>,
    /// The application (`id`, `flags`, `flags_new`).
    pub application: PartialApplication,
}

/// Raw form of an event without a typed variant.
#[derive(Debug, Clone)]
pub struct UnknownEvent {
    /// Event name (`t`).
    pub name: Box<str>,
    /// Event data (`d`) as raw JSON.
    pub data: Box<RawValue>,
}

impl Event {
    /// Builds an event from a dispatch's name and data. Fails only if a
    /// known event has data that does not match its model.
    pub fn from_dispatch(name: &str, data: Option<&RawValue>) -> serde_json::Result<Self> {
        if let Some(event) = Self::typed(name, data) {
            return event;
        }
        let data = match data {
            Some(raw) => raw.to_owned(),
            None => RawValue::from_string("null".to_owned())?,
        };
        Ok(Self::Unknown(UnknownEvent {
            name: name.into(),
            data,
        }))
    }

    /// Typed events; `None` for names without a typed variant.
    fn typed(name: &str, data: Option<&RawValue>) -> Option<serde_json::Result<Self>> {
        fn parse<'a, T: Deserialize<'a>>(data: Option<&'a RawValue>) -> serde_json::Result<T> {
            serde_json::from_str(data.map_or("null", RawValue::get))
        }

        let event = match name {
            "READY" => parse(data).map(Self::Ready),
            "RESUMED" => Ok(Self::Resumed),
            "GUILD_UPDATE" => parse(data).map(Self::GuildUpdate),
            "GUILD_DELETE" => parse(data).map(Self::GuildDelete),
            "CHANNEL_CREATE" => parse(data).map(Self::ChannelCreate),
            "CHANNEL_UPDATE" => parse(data).map(Self::ChannelUpdate),
            "CHANNEL_DELETE" => parse(data).map(Self::ChannelDelete),
            _ => return None,
        };
        Some(event)
    }

    /// The gateway name of the event (`t`).
    pub fn name(&self) -> &str {
        match self {
            Self::Ready(_) => "READY",
            Self::Resumed => "RESUMED",
            Self::GuildUpdate(_) => "GUILD_UPDATE",
            Self::GuildDelete(_) => "GUILD_DELETE",
            Self::ChannelCreate(_) => "CHANNEL_CREATE",
            Self::ChannelUpdate(_) => "CHANNEL_UPDATE",
            Self::ChannelDelete(_) => "CHANNEL_DELETE",
            Self::Unknown(event) => &event.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::payload::Envelope;

    #[test]
    fn ready_with_all_fields() {
        let json = r#"{"op":0,"s":1,"t":"READY","d":{
            "v": 10,
            "user": {"id":"80351110224678912","username":"zeny","discriminator":"0",
                     "global_name":null,"avatar":null,"bot":true},
            "guilds": [{"id":"41771983423143937","unavailable":true}],
            "session_id": "abc123",
            "resume_gateway_url": "wss://gateway-us-east1-b.discord.gg",
            "shard": [0, 2],
            "application": {"id":"80351110224678912","flags":8192,"flags_new":"8192"},
            "_trace": ["future field"]
        }}"#;
        let env = Envelope::parse(json).unwrap();
        let Event::Ready(ready) = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap()
        else {
            panic!("expected READY");
        };
        assert_eq!(ready.v, 10);
        assert!(ready.user.bot);
        assert!(ready.guilds[0].unavailable);
        assert_eq!(ready.shard, Some([0, 2]));
        assert_eq!(
            &*ready.resume_gateway_url,
            "wss://gateway-us-east1-b.discord.gg"
        );
        assert!(
            ready
                .application
                .all_flags()
                .contains(crate::application::ApplicationFlags::GATEWAY_PRESENCE_LIMITED)
        );
    }

    #[test]
    fn guild_delete_removed_vs_unavailable() {
        let removed = Event::from_dispatch(
            "GUILD_DELETE",
            Some(&RawValue::from_string(r#"{"id":"1"}"#.into()).unwrap()),
        )
        .unwrap();
        assert!(matches!(removed, Event::GuildDelete(g) if !g.unavailable));
    }

    #[test]
    fn known_event_with_bad_data_is_an_error() {
        let bad = RawValue::from_string(r#"{"id":"not a snowflake","type":0}"#.into()).unwrap();
        assert!(Event::from_dispatch("CHANNEL_CREATE", Some(&bad)).is_err());
    }

    #[test]
    fn resumed_and_unknown() {
        let env = Envelope::parse(r#"{"op":0,"s":2,"t":"RESUMED","d":{}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert!(matches!(event, Event::Resumed));

        let env = Envelope::parse(r#"{"op":0,"s":3,"t":"BRAND_NEW_EVENT","d":{"x":[1]}}"#).unwrap();
        let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
        assert_eq!(event.name(), "BRAND_NEW_EVENT");
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), r#"{"x":[1]}"#),
            other => panic!("expected unknown, got {}", other.name()),
        }
    }

    #[test]
    fn missing_data_becomes_null() {
        let event = Event::from_dispatch("SOMETHING", None).unwrap();
        match event {
            Event::Unknown(raw) => assert_eq!(raw.data.get(), "null"),
            other => panic!("expected unknown, got {}", other.name()),
        }
    }
}
