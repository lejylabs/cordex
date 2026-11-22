//! `GET /gateway/bot` response (events/gateway.mdx, "Get Gateway Bot").

use serde::{Deserialize, Serialize};

/// Recommended gateway URL and session-start budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GatewayBot {
    /// Websocket URL (`wss://gateway.discord.gg`).
    pub url: Box<str>,
    /// Recommended shard count.
    pub shards: u32,
    /// Identify budget.
    pub session_start_limit: SessionStartLimit,
}

impl GatewayBot {
    /// Build a `GET /gateway/bot` payload (tests and injected manager config).
    #[must_use]
    pub fn new(
        url: impl Into<Box<str>>,
        shards: u32,
        session_start_limit: SessionStartLimit,
    ) -> Self {
        Self {
            url: url.into(),
            shards,
            session_start_limit,
        }
    }
}

/// How many identifies the bot may still start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionStartLimit {
    /// Total identifies allowed per window.
    pub total: u32,
    /// Remaining identifies.
    pub remaining: u32,
    /// Milliseconds until `remaining` resets.
    pub reset_after: u64,
    /// Concurrent identify buckets (`shard_id % max_concurrency`).
    pub max_concurrency: u32,
}

impl SessionStartLimit {
    /// Build a session-start budget from `GET /gateway/bot`.
    #[must_use]
    pub const fn new(total: u32, remaining: u32, reset_after: u64, max_concurrency: u32) -> Self {
        Self {
            total,
            remaining,
            reset_after,
            max_concurrency,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_shape() {
        let json = r#"{"url":"wss://gateway.discord.gg","shards":9,"session_start_limit":{"total":1000,"remaining":999,"reset_after":14400000,"max_concurrency":16}}"#;
        let bot: GatewayBot = serde_json::from_str(json).unwrap();
        assert_eq!(&*bot.url, "wss://gateway.discord.gg");
        assert_eq!(bot.shards, 9);
        assert_eq!(bot.session_start_limit.max_concurrency, 16);
    }
}
