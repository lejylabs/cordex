//! Send-side payloads (events/gateway-events.mdx, "Send Events").
//!
//! Each command implements [`Command`]; [`Command::write_json`] serializes
//! `{ "op": …, "d": … }` into a caller-owned buffer, so the gateway can
//! reuse one buffer per connection (PLAN.md §16.2).

use serde::Serialize;

use super::intents::Intents;
use super::opcode::OpCode;
use super::presence::UpdatePresence;
use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use crate::serde_util::str_enum;

/// A payload the client sends to the gateway.
pub trait Command: Serialize {
    /// Opcode of the payload.
    const OP: OpCode;

    /// Appends `{"op":…,"d":…}` to `buf`.
    fn write_json(&self, buf: &mut Vec<u8>) -> serde_json::Result<()> {
        #[derive(Serialize)]
        struct Frame<'a, T: ?Sized> {
            op: OpCode,
            d: &'a T,
        }
        serde_json::to_writer(
            buf,
            &Frame {
                op: Self::OP,
                d: self,
            },
        )
    }
}

/// Opcode 1 Heartbeat: the last sequence number, `null` if none yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Heartbeat(pub Option<u64>);

impl Command for Heartbeat {
    const OP: OpCode = OpCode::Heartbeat;
}

/// Connection properties in Identify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ConnectionProperties<'a> {
    /// Operating system.
    pub os: &'a str,
    /// Library name.
    pub browser: &'a str,
    /// Library name.
    pub device: &'a str,
}

/// Opcode 2 Identify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Identify<'a> {
    /// Bot token (raw, from [`crate::Token::expose`]).
    pub token: &'a str,
    /// Connection properties.
    pub properties: ConnectionProperties<'a>,
    /// Payload compression; always off because transport compression
    /// (`zlib-stream`) is used instead (PLAN.md §8.5).
    pub compress: bool,
    /// 50–250: members above which offline members are not sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_threshold: Option<u8>,
    /// `[shard_id, num_shards]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shard: Option<[u32; 2]>,
    /// Initial presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<&'a UpdatePresence>,
    /// Intents.
    pub intents: Intents,
}

impl<'a> Identify<'a> {
    /// Identify with token, properties and intents; everything else default.
    pub const fn new(
        token: &'a str,
        properties: ConnectionProperties<'a>,
        intents: Intents,
    ) -> Self {
        Self {
            token,
            properties,
            compress: false,
            large_threshold: None,
            shard: None,
            presence: None,
            intents,
        }
    }
}

impl Command for Identify<'_> {
    const OP: OpCode = OpCode::Identify;
}

/// Opcode 3 Presence Update.
impl Command for UpdatePresence {
    const OP: OpCode = OpCode::PresenceUpdate;
}

/// Opcode 4 Voice State Update (payload only; this library has no voice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct UpdateVoiceState {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Voice channel to join; `None` (sent as `null`) disconnects.
    pub channel_id: Option<Id<ChannelMarker>>,
    /// Whether the client is muted.
    pub self_mute: bool,
    /// Whether the client is deafened.
    pub self_deaf: bool,
}

impl UpdateVoiceState {
    /// Voice state update for a guild.
    pub const fn new(
        guild_id: Id<GuildMarker>,
        channel_id: Option<Id<ChannelMarker>>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            self_mute,
            self_deaf,
        }
    }
}

impl Command for UpdateVoiceState {
    const OP: OpCode = OpCode::VoiceStateUpdate;
}

/// Opcode 6 Resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Resume<'a> {
    /// Bot token.
    pub token: &'a str,
    /// Session ID from Ready.
    pub session_id: &'a str,
    /// Last sequence number received.
    pub seq: u64,
}

impl<'a> Resume<'a> {
    /// Resume payload.
    pub const fn new(token: &'a str, session_id: &'a str, seq: u64) -> Self {
        Self {
            token,
            session_id,
            seq,
        }
    }
}

impl Command for Resume<'_> {
    const OP: OpCode = OpCode::Resume;
}

/// Maximum nonce length in bytes for Request Guild Members.
pub const MEMBER_NONCE_MAX_LEN: usize = 32;

/// Opcode 8 Request Guild Members. Use the constructors; they only allow
/// the field combinations Discord accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestGuildMembers<'a> {
    /// Guild (one per request).
    pub guild_id: Id<GuildMarker>,
    /// Username prefix; empty with `limit` 0 requests all members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<&'a str>,
    /// Maximum number of members; required with `query`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Include presences (needs `GUILD_PRESENCES`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub presences: bool,
    /// Users to fetch (max. 100 returned).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<&'a [Id<UserMarker>]>,
    /// Nonce echoed in the Guild Members Chunk events (max. 32 bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<&'a str>,
}

impl<'a> RequestGuildMembers<'a> {
    /// All members (`query: ""`, `limit: 0`). Needs the `GUILD_MEMBERS` intent.
    pub const fn all(guild_id: Id<GuildMarker>) -> Self {
        Self::query(guild_id, "", 0)
    }

    /// Members whose username starts with `query` (max. 100 returned).
    pub const fn query(guild_id: Id<GuildMarker>, query: &'a str, limit: u32) -> Self {
        Self {
            guild_id,
            query: Some(query),
            limit: Some(limit),
            presences: false,
            user_ids: None,
            nonce: None,
        }
    }

    /// Specific users (max. 100 returned).
    pub const fn user_ids(guild_id: Id<GuildMarker>, user_ids: &'a [Id<UserMarker>]) -> Self {
        Self {
            guild_id,
            query: None,
            limit: None,
            presences: false,
            user_ids: Some(user_ids),
            nonce: None,
        }
    }

    /// Requests presences too.
    #[must_use]
    pub const fn presences(mut self) -> Self {
        self.presences = true;
        self
    }

    /// Sets the nonce; `None` if it is longer than 32 bytes (Discord would
    /// silently ignore it).
    #[must_use]
    pub const fn nonce(mut self, nonce: &'a str) -> Option<Self> {
        if nonce.len() > MEMBER_NONCE_MAX_LEN {
            return None;
        }
        self.nonce = Some(nonce);
        Some(self)
    }
}

impl Command for RequestGuildMembers<'_> {
    const OP: OpCode = OpCode::RequestGuildMembers;
}

/// Opcode 31 Request Soundboard Sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestSoundboardSounds<'a> {
    /// Guilds to get soundboard sounds for.
    pub guild_ids: &'a [Id<GuildMarker>],
}

impl<'a> RequestSoundboardSounds<'a> {
    /// Request for the given guilds.
    pub const fn new(guild_ids: &'a [Id<GuildMarker>]) -> Self {
        Self { guild_ids }
    }
}

impl Command for RequestSoundboardSounds<'_> {
    const OP: OpCode = OpCode::RequestSoundboardSounds;
}

str_enum! {
    /// Field requested with Request Channel Info.
    pub enum ChannelInfoField {
        /// Voice channel status.
        Status = "status",
        /// Voice channel start time.
        VoiceStartTime = "voice_start_time",
    }
}

/// Opcode 43 Request Channel Info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestChannelInfo<'a> {
    /// Guild.
    pub guild_id: Id<GuildMarker>,
    /// Fields to request.
    pub fields: &'a [ChannelInfoField],
}

impl<'a> RequestChannelInfo<'a> {
    /// Request for a guild.
    pub const fn new(guild_id: Id<GuildMarker>, fields: &'a [ChannelInfoField]) -> Self {
        Self { guild_id, fields }
    }
}

impl Command for RequestChannelInfo<'_> {
    const OP: OpCode = OpCode::RequestChannelInfo;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::presence::{Activity, ActivityType, Status};

    fn json<C: Command>(cmd: &C) -> String {
        let mut buf = Vec::new();
        cmd.write_json(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    fn guild(id: u64) -> Id<GuildMarker> {
        Id::new(id).unwrap()
    }

    #[test]
    fn heartbeat() {
        assert_eq!(json(&Heartbeat(Some(251))), r#"{"op":1,"d":251}"#);
        assert_eq!(json(&Heartbeat(None)), r#"{"op":1,"d":null}"#);
    }

    #[test]
    fn identify_like_the_docs_example() {
        let presence = UpdatePresence {
            since: Some(91_879_201),
            activities: vec![Activity::new(
                ActivityType::Playing,
                "Cards Against Humanity",
            )],
            status: Status::DoNotDisturb,
            afk: false,
        };
        let mut identify = Identify::new(
            "my_token",
            ConnectionProperties {
                os: "linux",
                browser: "disco",
                device: "disco",
            },
            Intents::GUILDS | Intents::GUILD_MODERATION,
        );
        identify.large_threshold = Some(250);
        identify.shard = Some([0, 1]);
        identify.presence = Some(&presence);
        assert_eq!(
            json(&identify),
            concat!(
                r#"{"op":2,"d":{"token":"my_token","#,
                r#""properties":{"os":"linux","browser":"disco","device":"disco"},"#,
                r#""compress":false,"large_threshold":250,"shard":[0,1],"#,
                r#""presence":{"since":91879201,"activities":[{"name":"Cards Against Humanity","type":0}],"status":"dnd","afk":false},"#,
                r#""intents":5}}"#
            )
        );
    }

    #[test]
    fn resume() {
        assert_eq!(
            json(&Resume::new("randomstring", "evenmorerandomstring", 1337)),
            r#"{"op":6,"d":{"token":"randomstring","session_id":"evenmorerandomstring","seq":1337}}"#
        );
    }

    #[test]
    fn request_guild_members_variants() {
        assert_eq!(
            json(&RequestGuildMembers::all(guild(41_771_983_444_115_456))),
            r#"{"op":8,"d":{"guild_id":"41771983444115456","query":"","limit":0}}"#
        );
        let users = [Id::new(1).unwrap(), Id::new(2).unwrap()];
        let req = RequestGuildMembers::user_ids(guild(5), &users)
            .presences()
            .nonce("abc")
            .unwrap();
        assert_eq!(
            json(&req),
            r#"{"op":8,"d":{"guild_id":"5","presences":true,"user_ids":["1","2"],"nonce":"abc"}}"#
        );
        assert!(
            RequestGuildMembers::all(guild(5))
                .nonce(&"x".repeat(33))
                .is_none()
        );
    }

    #[test]
    fn voice_state_null_channel() {
        assert_eq!(
            json(&UpdateVoiceState::new(
                guild(41_771_983_423_143_937),
                None,
                false,
                true
            )),
            r#"{"op":4,"d":{"guild_id":"41771983423143937","channel_id":null,"self_mute":false,"self_deaf":true}}"#
        );
    }

    #[test]
    fn soundboard_and_channel_info() {
        assert_eq!(
            json(&RequestSoundboardSounds::new(&[guild(1), guild(2)])),
            r#"{"op":31,"d":{"guild_ids":["1","2"]}}"#
        );
        assert_eq!(
            json(&RequestChannelInfo::new(
                guild(1),
                &[ChannelInfoField::Status, ChannelInfoField::VoiceStartTime]
            )),
            r#"{"op":43,"d":{"guild_id":"1","fields":["status","voice_start_time"]}}"#
        );
    }

    #[test]
    fn presence_update() {
        assert_eq!(
            json(&UpdatePresence::new(Status::Idle)),
            r#"{"op":3,"d":{"since":null,"activities":[],"status":"idle","afk":false}}"#
        );
    }
}
