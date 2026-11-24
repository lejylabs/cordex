//! Lavalink v4 REST and WebSocket data types.

use std::fmt;

use cordex_model::id::Id;
use cordex_model::id::marker::{GuildMarker, UserMarker};
use cordex_voice::LavalinkVoiceState;
use serde::{Deserialize, Serialize};

/// Connection configuration for a Lavalink v4 node.
#[derive(Clone, PartialEq, Eq)]
pub struct NodeConfig {
    /// Hostname or IP address (e.g. `"127.0.0.1"` or `"lava.example.com"`).
    pub host: Box<str>,
    /// Port (default `2333`).
    pub port: u16,
    /// Lavalink server password (`Authorization` header).
    pub password: Box<str>,
    /// Use `https://` and `wss://` instead of `http://` and `ws://`.
    pub secure: bool,
    /// Bot's Discord user ID (`User-Id` header).
    pub user_id: Id<UserMarker>,
    /// Client name sent in `Client-Name` header.
    pub client_name: Box<str>,
}

impl NodeConfig {
    /// Create a new node configuration with default client name and `secure: false`.
    #[must_use]
    pub fn new(
        host: impl Into<Box<str>>,
        port: u16,
        password: impl Into<Box<str>>,
        user_id: Id<UserMarker>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            password: password.into(),
            secure: false,
            user_id,
            client_name: concat!("cordex-lavalink/", env!("CARGO_PKG_VERSION")).into(),
        }
    }

    /// Enable or disable TLS (`https://` / `wss://`).
    #[must_use]
    pub const fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Override the `Client-Name` header.
    #[must_use]
    pub fn client_name(mut self, name: impl Into<Box<str>>) -> Self {
        self.client_name = name.into();
        self
    }

    /// Base HTTP URL (`http(s)://{host}:{port}`).
    #[must_use]
    pub fn http_base_url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{scheme}://{}:{}", self.host, self.port)
    }

    /// WebSocket URL (`ws(s)://{host}:{port}/v4/websocket`).
    #[must_use]
    pub fn ws_url(&self) -> String {
        let scheme = if self.secure { "wss" } else { "ws" };
        format!("{scheme}://{}:{}/v4/websocket", self.host, self.port)
    }
}

impl fmt::Debug for NodeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("password", &"***")
            .field("secure", &self.secure)
            .field("user_id", &self.user_id)
            .field("client_name", &self.client_name)
            .finish()
    }
}

/// A Lavalink v4 track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Track {
    /// Base64-encoded track data used to play the track.
    pub encoded: Box<str>,
    /// Track metadata.
    pub info: TrackInfo,
}

/// Metadata of a Lavalink v4 [`Track`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TrackInfo {
    /// Source-specific track identifier.
    pub identifier: Box<str>,
    /// Whether the track is seekable.
    pub is_seekable: bool,
    /// Track author.
    pub author: Box<str>,
    /// Track duration in milliseconds.
    pub length: u64,
    /// Whether the track is a live stream.
    pub is_stream: bool,
    /// Current playback position in milliseconds.
    pub position: u64,
    /// Track title.
    pub title: Box<str>,
    /// Track URI, if available.
    pub uri: Option<Box<str>>,
    /// Artwork URL, if available.
    pub artwork_url: Option<Box<str>>,
    /// ISRC code, if available.
    pub isrc: Option<Box<str>>,
    /// Source name (e.g. `"youtube"`, `"http"`, `"soundcloud"`).
    pub source_name: Box<str>,
}

/// Playlist metadata returned by `GET /v4/loadtracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PlaylistInfo {
    /// Playlist name.
    pub name: Box<str>,
    /// Selected track index, or `-1` if none.
    pub selected_track: i32,
}

/// Playlist payload returned by `GET /v4/loadtracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlaylistData {
    /// Playlist metadata.
    pub info: PlaylistInfo,
    /// Tracks in the playlist.
    pub tracks: Vec<Track>,
}

/// Error details returned when `loadType` is `"error"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoadError {
    /// Error message.
    pub message: Option<Box<str>>,
    /// Severity (`"common"`, `"suspicious"`, `"fault"`).
    pub severity: Box<str>,
    /// Cause description.
    pub cause: Box<str>,
}

/// Result of `GET /v4/loadtracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "loadType", content = "data", rename_all = "camelCase")]
#[non_exhaustive]
pub enum LoadResult {
    /// A single track was loaded.
    Track(Box<Track>),
    /// A playlist was loaded.
    Playlist(PlaylistData),
    /// Search results were returned.
    Search(Vec<Track>),
    /// Nothing matched the identifier.
    Empty {},
    /// Loading failed.
    Error(LoadError),
}

/// Live playback state of a Lavalink player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlayerState {
    /// Unix timestamp in milliseconds.
    pub time: u64,
    /// Track position in milliseconds.
    pub position: u64,
    /// Whether Lavalink is connected to the Discord voice gateway.
    pub connected: bool,
    /// Voice server ping in milliseconds (`-1` if not connected).
    pub ping: i64,
}

/// A Lavalink v4 guild player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Player {
    /// Guild ID.
    pub guild_id: Id<GuildMarker>,
    /// Currently playing track, if any.
    pub track: Option<Track>,
    /// Volume (`0..=1000`, default `100`).
    pub volume: u16,
    /// Whether playback is paused.
    pub paused: bool,
    /// Playback state.
    pub state: PlayerState,
    /// Current voice credentials.
    pub voice: LavalinkVoiceState,
}

/// Track selection in [`UpdatePlayer`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlayerTrack {
    /// Base64-encoded track, or `None` to stop playback.
    pub encoded: Option<Box<str>>,
}

/// Request body for `PATCH /v4/sessions/{sessionId}/players/{guildId}`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlayer {
    /// Track to play (or `encoded: None` to stop).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<UpdatePlayerTrack>,
    /// Seek position in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u64>,
    /// End time in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<u64>,
    /// Volume (`0..=1000`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u16>,
    /// Pause or resume playback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    /// Discord voice credentials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<LavalinkVoiceState>,
    /// If `true`, do not replace an already playing track (sent as query param).
    #[serde(skip)]
    pub no_replace: bool,
}

impl UpdatePlayer {
    /// Empty player update.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Play a base64-encoded track.
    #[must_use]
    pub fn encoded_track(mut self, encoded: impl Into<Box<str>>) -> Self {
        self.track = Some(UpdatePlayerTrack {
            encoded: Some(encoded.into()),
        });
        self
    }

    /// Stop the current track (`track: { encoded: null }`).
    #[must_use]
    pub fn stop_track(mut self) -> Self {
        self.track = Some(UpdatePlayerTrack { encoded: None });
        self
    }

    /// Set playback position in milliseconds.
    #[must_use]
    pub const fn position(mut self, position_ms: u64) -> Self {
        self.position = Some(position_ms);
        self
    }

    /// Set volume (`0..=1000`).
    #[must_use]
    pub const fn volume(mut self, volume: u16) -> Self {
        self.volume = Some(volume);
        self
    }

    /// Pause or resume playback.
    #[must_use]
    pub const fn paused(mut self, paused: bool) -> Self {
        self.paused = Some(paused);
        self
    }

    /// Set Discord voice credentials.
    #[must_use]
    pub fn voice(mut self, voice: LavalinkVoiceState) -> Self {
        self.voice = Some(voice);
        self
    }

    /// Set `noReplace` query parameter.
    #[must_use]
    pub const fn no_replace(mut self, no_replace: bool) -> Self {
        self.no_replace = no_replace;
        self
    }
}

/// Lavalink node statistics (`op: "stats"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NodeStats {
    /// Total players on the node.
    pub players: u32,
    /// Actively playing players.
    pub playing_players: u32,
    /// Node uptime in milliseconds.
    pub uptime: u64,
}

/// Player events dispatched inside `op: "event"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum PlayerEvent {
    /// A track started playing.
    #[serde(rename = "TrackStartEvent", rename_all = "camelCase")]
    TrackStart {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Started track.
        track: Box<Track>,
    },
    /// A track finished or was stopped/replaced.
    #[serde(rename = "TrackEndEvent", rename_all = "camelCase")]
    TrackEnd {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Ended track.
        track: Box<Track>,
        /// End reason (`"finished"`, `"loadFailed"`, `"stopped"`, `"replaced"`, `"cleanup"`).
        reason: Box<str>,
    },
    /// Playback threw an exception.
    #[serde(rename = "TrackExceptionEvent", rename_all = "camelCase")]
    TrackException {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Affected track.
        track: Box<Track>,
        /// Exception details.
        exception: LoadError,
    },
    /// Track audio frames got stuck.
    #[serde(rename = "TrackStuckEvent", rename_all = "camelCase")]
    TrackStuck {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Affected track.
        track: Box<Track>,
        /// Threshold in milliseconds.
        threshold_ms: u64,
    },
    /// Discord voice WebSocket closed.
    #[serde(rename = "WebSocketClosedEvent", rename_all = "camelCase")]
    WebSocketClosed {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Close code.
        code: u16,
        /// Close reason.
        reason: Box<str>,
        /// Whether closed by Discord.
        by_remote: bool,
    },
}

/// Incoming Lavalink v4 WebSocket message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
#[non_exhaustive]
pub enum LavalinkEvent {
    /// Session established; contains `sessionId` for REST calls.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Whether a previous session was resumed.
        resumed: bool,
        /// Session ID to use in REST URLs.
        session_id: Box<str>,
    },
    /// Periodic player state update.
    #[serde(rename_all = "camelCase")]
    PlayerUpdate {
        /// Guild ID.
        guild_id: Id<GuildMarker>,
        /// Updated player state.
        state: PlayerState,
    },
    /// Node resource statistics.
    Stats(NodeStats),
    /// Player lifecycle event.
    Event(PlayerEvent),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ready_event_deserializes() {
        let json = r#"{"op":"ready","resumed":false,"sessionId":"abc123"}"#;
        let ev: LavalinkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            ev,
            LavalinkEvent::Ready {
                resumed: false,
                session_id: "abc123".into(),
            }
        );
    }

    #[test]
    fn load_result_empty_and_search() {
        let empty: LoadResult = serde_json::from_str(r#"{"loadType":"empty","data":{}}"#).unwrap();
        assert_eq!(empty, LoadResult::Empty {});

        let search_json = r#"{
            "loadType": "search",
            "data": [{
                "encoded": "QAAAjQIA",
                "info": {
                    "identifier": "dQw4w9WgXcQ",
                    "isSeekable": true,
                    "author": "Rick Astley",
                    "length": 212000,
                    "isStream": false,
                    "position": 0,
                    "title": "Never Gonna Give You Up",
                    "uri": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    "artworkUrl": null,
                    "isrc": null,
                    "sourceName": "youtube"
                }
            }]
        }"#;
        let res: LoadResult = serde_json::from_str(search_json).unwrap();
        let LoadResult::Search(tracks) = res else {
            panic!("expected Search");
        };
        assert_eq!(tracks.len(), 1);
        assert_eq!(&*tracks[0].info.title, "Never Gonna Give You Up");
    }

    #[test]
    fn update_player_serializes_cleanly() {
        let body = UpdatePlayer::new()
            .encoded_track("QAAAjQIA")
            .volume(80)
            .paused(false);
        let val = serde_json::to_value(&body).unwrap();
        assert_eq!(
            val,
            serde_json::json!({
                "track": { "encoded": "QAAAjQIA" },
                "volume": 80,
                "paused": false
            })
        );
    }

    #[test]
    fn node_config_redacts_password() {
        let cfg = NodeConfig::new("127.0.0.1", 2333, "super-secret", Id::new(1).unwrap());
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains("super-secret"));
        assert!(dbg.contains("***"));
        assert_eq!(cfg.http_base_url(), "http://127.0.0.1:2333");
        assert_eq!(cfg.ws_url(), "ws://127.0.0.1:2333/v4/websocket");
    }
}
