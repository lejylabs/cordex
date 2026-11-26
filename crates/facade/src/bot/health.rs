//! Runtime health snapshot for Kubernetes probes, dashboards, and monitoring.

use cordex_gateway::ShardState;

/// Point-in-time health and diagnostics snapshot of a running [`Bot`](super::Bot).
///
/// Always available with zero extra dependencies; call [`Self::to_json`] to
/// serialize directly for `/healthz` or `/readyz` endpoints.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HealthSnapshot {
    /// Per-shard gateway status, latency, and reconnect counters.
    pub shards: Vec<ShardHealth>,
    /// Number of currently active `Standby` event waiters.
    pub standby_waiters: usize,
    /// Total HTTP requests sent by the bot's REST client.
    pub http_requests: u64,
    /// Total HTTP 429 rate-limit responses observed.
    pub http_rate_limited: u64,
    /// Cache entry counts, when the `cache` feature is enabled and a cache is attached.
    #[cfg(feature = "cache")]
    pub cache: Option<CacheHealth>,
}

impl HealthSnapshot {
    /// Returns `true` when at least one shard is [`ShardState::Active`] and
    /// no shard is disconnected or shutting down.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        !self.shards.is_empty()
            && self
                .shards
                .iter()
                .all(|s| matches!(s.state, ShardState::Active))
    }

    /// Render this snapshot as a JSON object suitable for `/healthz` or `/readyz`.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let shards: Vec<serde_json::Value> = self
            .shards
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "state": shard_state_label(s.state),
                    "latency_ms": s.latency_ms,
                    "last_heartbeat_ack_ms": s.last_heartbeat_ack_ms,
                    "reconnect_count": s.reconnect_count,
                    "guild_count": s.guild_count,
                })
            })
            .collect();
        #[allow(unused_mut)]
        let mut obj = serde_json::json!({
            "ready": self.is_ready(),
            "shards": shards,
            "standby_waiters": self.standby_waiters,
            "http_requests": self.http_requests,
            "http_rate_limited": self.http_rate_limited,
        });
        #[cfg(feature = "cache")]
        if let Some(cache) = self.cache
            && let Some(map) = obj.as_object_mut()
        {
            map.insert(
                "cache".to_owned(),
                serde_json::json!({
                    "guilds": cache.guilds,
                    "channels": cache.channels,
                    "threads": cache.threads,
                    "users": cache.users,
                    "members": cache.members,
                    "messages": cache.messages,
                }),
            );
        }
        obj
    }
}

/// Health metrics for a single gateway shard.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ShardHealth {
    /// Zero-based shard number.
    pub id: u32,
    /// Current shard connection state.
    pub state: ShardState,
    /// Most recent heartbeat round-trip latency in milliseconds, if known.
    pub latency_ms: Option<u64>,
    /// Milliseconds elapsed since the last heartbeat ACK, if any.
    pub last_heartbeat_ack_ms: Option<u64>,
    /// Number of reconnects since startup.
    pub reconnect_count: u32,
    /// Number of guilds currently reported by this shard.
    pub guild_count: u32,
}

/// Summary of in-memory cache sizes for [`HealthSnapshot`].
#[cfg(feature = "cache")]
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CacheHealth {
    /// Cached guilds.
    pub guilds: usize,
    /// Cached channels (excluding threads).
    pub channels: usize,
    /// Cached threads.
    pub threads: usize,
    /// Cached users.
    pub users: usize,
    /// Cached members across all guilds.
    pub members: usize,
    /// Cached messages across all channels.
    pub messages: usize,
}

const fn shard_state_label(state: ShardState) -> &'static str {
    match state {
        ShardState::Disconnected => "disconnected",
        ShardState::Connecting => "connecting",
        ShardState::AwaitingHello => "awaiting_hello",
        ShardState::Identifying => "identifying",
        ShardState::Resuming => "resuming",
        ShardState::Active => "active",
        ShardState::Reconnecting => "reconnecting",
        ShardState::ShuttingDown => "shutting_down",
    }
}
