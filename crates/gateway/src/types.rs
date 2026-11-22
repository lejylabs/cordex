//! Shard identity, connection state and backpressure (PLAN.md §8, §10).

use std::time::Duration;

use drs_model::gateway::event::Event;

use crate::error::GatewayError;

/// Library name sent as Identify `browser` and `device`.
pub const LIBRARY_NAME: &str = "drs";

/// Default gateway host; query parameters are added by the shard.
pub const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg";

/// Discord allows 120 gateway sends per 60 seconds per connection.
pub const OUTBOUND_WINDOW: Duration = Duration::from_secs(60);
/// Total send tokens in [`OUTBOUND_WINDOW`].
pub const OUTBOUND_LIMIT: u32 = 120;
/// Tokens reserved for heartbeats (user payloads may use the rest).
pub const HEARTBEAT_RESERVE: u32 = 10;
/// User payloads may use at most this many sends per window.
pub const USER_OUTBOUND_LIMIT: u32 = OUTBOUND_LIMIT - HEARTBEAT_RESERVE;
/// Discord closes with 4008 if a send exceeds this.
pub const MAX_PAYLOAD_BYTES: usize = 4096;

pub(crate) const HELLO_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) const READY_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const STABLE_ACTIVE: Duration = Duration::from_secs(60);
pub(crate) const DEFAULT_EVENT_CAPACITY: usize = 1024;
pub(crate) const DEFAULT_COMMAND_CAPACITY: usize = 32;
pub(crate) const DEFAULT_LARGE_THRESHOLD: u8 = 50;
pub(crate) const DEFAULT_MEMBER_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const LATENCY_SAMPLES: usize = 5;
pub(crate) const PANIC_WINDOW: Duration = Duration::from_secs(600);
pub(crate) const PANIC_LIMIT: u32 = 5;

/// Which connection of a sharded bot this shard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardId {
    id: u32,
    total: u32,
}

impl ShardId {
    /// `id` of `total` shards. `id` must be less than `total`, and `total`
    /// must be at least 1.
    pub fn new(id: u32, total: u32) -> Result<Self, GatewayError> {
        if total == 0 || id >= total {
            return Err(GatewayError::InvalidShardId { id, total });
        }
        Ok(Self { id, total })
    }

    /// Single-shard bot (`0/1`).
    #[must_use]
    pub const fn one() -> Self {
        Self { id: 0, total: 1 }
    }

    /// Shard number (`0..total`).
    #[must_use]
    pub const fn number(self) -> u32 {
        self.id
    }

    /// Total shard count.
    #[must_use]
    pub const fn total(self) -> u32 {
        self.total
    }

    /// Pair sent in Identify (`[id, count]`).
    #[must_use]
    pub const fn identify_pair(self) -> [u32; 2] {
        [self.id, self.total]
    }
}

impl Default for ShardId {
    fn default() -> Self {
        Self::one()
    }
}

/// Observable connection state (PLAN.md §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShardState {
    /// No socket.
    Disconnected,
    /// Opening the WebSocket.
    Connecting,
    /// Socket is up; waiting for opcode 10.
    AwaitingHello,
    /// Identify sent; waiting for READY.
    Identifying,
    /// Resume sent; waiting for RESUMED (or a new READY).
    Resuming,
    /// Receiving dispatches.
    Active,
    /// Backing off before the next connect.
    Reconnecting,
    /// User-requested close in progress.
    ShuttingDown,
}

/// How to close the gateway connection (PLAN.md §8.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloseMode {
    /// Close with 4000 and keep session data so another process can resume.
    Resumable,
    /// Close with 1000 and invalidate the session.
    Terminate,
}

/// Session data needed to resume after a deploy or a resumable close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeInfo {
    /// Session id from READY.
    pub session_id: Box<str>,
    /// Last dispatch sequence number.
    pub seq: u64,
    /// `resume_gateway_url` from READY.
    pub resume_gateway_url: Box<str>,
}

/// What the shard does when the event buffer is full (PLAN.md §10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Backpressure {
    /// Wait until the user reads an event. Heartbeats keep running.
    Block,
    /// Drop the oldest buffered event.
    DropOldest,
    /// Drop the event that did not fit.
    DropNewest,
    /// Close with a resumable code; Discord will replay (default).
    #[default]
    Disconnect,
}

/// Fan-in item from [`crate::ShardManager::events`] (PLAN.md §9.4).
#[derive(Debug)]
#[non_exhaustive]
pub enum ManagerEvent {
    /// A typed (or unknown) dispatch from one shard.
    Dispatch {
        /// Shard that received the event.
        shard: ShardId,
        /// Dispatch payload.
        event: Event,
    },
    /// The shard stopped with a fatal error (no reconnect).
    Error {
        /// Shard that failed.
        shard: ShardId,
        /// Fatal error.
        error: GatewayError,
    },
    /// Connection state machine transition.
    StateChanged {
        /// Shard that changed.
        shard: ShardId,
        /// Previous state.
        old: ShardState,
        /// New state.
        new: ShardState,
    },
    /// `/gateway/bot` recommended more shards than this process is running.
    RecommendedShardCount {
        /// Shards started by this manager.
        running: u32,
        /// `shards` field from `GET /gateway/bot`.
        recommended: u32,
    },
}

/// Snapshot of one shard for [`crate::ShardManager::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardStatus {
    /// Shard identity.
    pub id: ShardId,
    /// Connection state machine.
    pub state: ShardState,
    /// Heartbeat round-trips.
    pub latency: Latency,
    /// Time since the last heartbeat ACK.
    pub last_heartbeat_ack: Option<Duration>,
    /// Guilds reported in READY.
    pub guild_count: u32,
    /// Successful reconnects after the first connection.
    pub reconnect_count: u32,
}

/// Recent heartbeat round-trips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latency {
    /// Newest sample (send → ACK).
    pub latest: Option<Duration>,
    /// Up to the last five samples, oldest first.
    pub recent: Vec<Duration>,
}

impl Latency {
    pub(crate) fn new() -> Self {
        Self {
            latest: None,
            recent: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, sample: Duration) {
        self.latest = Some(sample);
        self.recent.push(sample);
        if self.recent.len() > LATENCY_SAMPLES {
            self.recent.remove(0);
        }
    }
}

/// Attach the v10 JSON zlib-stream query string to a gateway base URL.
pub(crate) fn with_gateway_query(url: &str) -> String {
    let without_query = url.split_once('?').map_or(url, |(head, _)| head);
    let without_query = without_query.trim_end_matches('/');
    format!(
        "{without_query}/?v={}&encoding=json&compress=zlib-stream",
        drs_model::API_VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_id_rejects_out_of_range() {
        assert!(ShardId::new(0, 0).is_err());
        assert!(ShardId::new(1, 1).is_err());
        assert!(ShardId::new(0, 1).is_ok());
        assert_eq!(ShardId::new(3, 4).unwrap().identify_pair(), [3, 4]);
    }

    #[test]
    fn query_replaces_existing() {
        assert_eq!(
            with_gateway_query("wss://gateway.discord.gg"),
            "wss://gateway.discord.gg/?v=10&encoding=json&compress=zlib-stream"
        );
        assert_eq!(
            with_gateway_query("wss://resume.discord.gg/?foo=1"),
            "wss://resume.discord.gg/?v=10&encoding=json&compress=zlib-stream"
        );
    }
}
