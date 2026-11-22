//! Fill the cache with data that existed before the bot started.
//!
//! The gateway already sends the full current state of every guild at
//! startup (`GUILD_CREATE`: channels, roles, emojis, voice states, …), so
//! those resources need no backfill. Two things are missing:
//!
//! - **Messages**: the gateway never sends history. [`messages`] and
//!   [`guild_messages`] load it over REST.
//! - **Members**: without the privileged `GUILD_MEMBERS` intent Discord
//!   sends only the bot itself. With the intent, [`guild_members`] requests
//!   every member over the gateway.
//!
//! Nothing here runs by itself: the cache stays free of network access and
//! a backfill only happens when you call these functions. They respect the
//! cache configuration (switches, parts, `message_cache_size`, member cap)
//! and go through the normal REST rate limiter.
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use cordex::backfill::{self, BackfillOptions};
//! # use cordex::prelude::*;
//! # async fn run(http: Client, cache: Arc<InMemoryCache>, guild: Id<cordex::model::id::marker::GuildMarker>) {
//! // After the guild's GUILD_CREATE went through `cache.update(...)`:
//! let report = backfill::guild_messages(&http, &cache, guild, BackfillOptions::new()).await;
//! println!("{} messages in {} channels", report.messages, report.channels);
//! # }
//! ```

use std::sync::Arc;

use cordex_cache::{InMemoryCache, ResourceType};
use cordex_http::{Client, HttpError};
use cordex_model::channel::ChannelType;
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Messages per REST call (Discord maximum).
const PAGE: usize = 100;

/// Options for [`guild_messages`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackfillOptions {
    concurrency: usize,
    threads: bool,
}

impl Default for BackfillOptions {
    fn default() -> Self {
        Self {
            concurrency: 4,
            threads: true,
        }
    }
}

impl BackfillOptions {
    /// Defaults: 4 channels at a time, threads included.
    pub fn new() -> Self {
        Self::default()
    }

    /// Channels loaded at the same time (min. 1). Each channel has its own
    /// rate limit bucket, so a few in parallel are faster without hitting
    /// limits; the rate limiter still paces every request.
    #[must_use]
    pub fn concurrency(mut self, channels: usize) -> Self {
        self.concurrency = channels.max(1);
        self
    }

    /// Include cached threads (default `true`).
    #[must_use]
    pub fn threads(mut self, include: bool) -> Self {
        self.threads = include;
        self
    }
}

/// Result of [`guild_messages`].
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BackfillReport {
    /// Channels loaded successfully.
    pub channels: usize,
    /// Messages stored in the cache.
    pub messages: usize,
    /// Channels that failed, e.g. `403` without "Read Message History".
    pub skipped: Vec<(Id<ChannelMarker>, HttpError)>,
}

/// Load the newest messages of one channel into the cache, up to the
/// configured `message_cache_size` (paged by 100). Returns how many new
/// messages were stored; `0` without a request when messages are off.
pub async fn messages(
    http: &Client,
    cache: &InMemoryCache,
    channel: Id<ChannelMarker>,
) -> Result<usize, HttpError> {
    let config = cache.config();
    let wanted = config.messages_per_channel();
    if !config.has(ResourceType::MESSAGES) || wanted == 0 {
        return Ok(0);
    }
    let mut fetched = 0;
    let mut stored = 0;
    let mut before: Option<Id<MessageMarker>> = None;
    while fetched < wanted {
        let limit = (wanted - fetched).min(PAGE);
        let mut request = http
            .get_channel_messages(channel)
            .limit(u8::try_from(limit).unwrap_or(100));
        if let Some(id) = before {
            request = request.before(id);
        }
        let page = request.send().await?;
        let count = page.len();
        before = page.iter().map(|m| m.id).min();
        fetched += count;
        stored += cache.insert_messages(channel, page);
        if count < limit {
            break;
        }
    }
    Ok(stored)
}

/// Load messages for every text channel of a cached guild (and its threads,
/// see [`BackfillOptions::threads`]). Needs the guild's channels in the
/// cache ([`ResourceType::CHANNELS`] / [`ResourceType::THREADS`]), so call
/// it after the guild's `GUILD_CREATE`. Failing channels are reported in
/// [`BackfillReport::skipped`] and do not stop the others.
pub async fn guild_messages(
    http: &Client,
    cache: &Arc<InMemoryCache>,
    guild: Id<GuildMarker>,
    options: BackfillOptions,
) -> BackfillReport {
    let mut report = BackfillReport::default();
    let config = cache.config();
    if !config.has(ResourceType::MESSAGES) || config.messages_per_channel() == 0 {
        return report;
    }
    let mut channels: Vec<Id<ChannelMarker>> = cache
        .guild_channels(guild)
        .iter()
        .filter(|c| has_messages(c.kind))
        .map(|c| c.id)
        .collect();
    if options.threads {
        channels.extend(cache.guild_threads(guild).iter().map(|t| t.id));
    }

    let permits = Arc::new(Semaphore::new(options.concurrency));
    let mut tasks = JoinSet::new();
    for channel in channels {
        let (http, cache, permits) = (http.clone(), Arc::clone(cache), Arc::clone(&permits));
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await;
            (channel, messages(&http, &cache, channel).await)
        });
    }
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, Ok(n))) => {
                report.channels += 1;
                report.messages += n;
            }
            Ok((channel, Err(err))) => report.skipped.push((channel, err)),
            // Only a panic inside the request code ends a task this way;
            // its message is already printed, the other channels go on.
            Err(_) => {}
        }
    }
    report
}

/// Request every member of a guild over the gateway and store them. Needs
/// the privileged `GUILD_MEMBERS` intent (enable it in the developer portal
/// and in the shard's intents) and [`ResourceType::MEMBERS`]. Returns how
/// many members of the guild the cache holds afterwards.
#[cfg(feature = "gateway")]
pub async fn guild_members(
    shard: &cordex_gateway::Shard,
    cache: &InMemoryCache,
    guild: Id<GuildMarker>,
) -> Result<usize, cordex_gateway::GatewayError> {
    if !cache.config().has(ResourceType::MEMBERS) {
        return Ok(0);
    }
    let members = shard.request_all_members(guild).await?;
    Ok(cache.insert_members(guild, members))
}

/// Channel types that hold messages (text in voice/stage included; forum
/// and media posts are threads).
fn has_messages(kind: ChannelType) -> bool {
    matches!(
        kind,
        ChannelType::GuildText
            | ChannelType::GuildAnnouncement
            | ChannelType::GuildVoice
            | ChannelType::GuildStageVoice
            | ChannelType::AnnouncementThread
            | ChannelType::PublicThread
            | ChannelType::PrivateThread
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use cordex_cache::{CacheConfig, CacheParts};
    use cordex_http::mock::{self, MockResponse};
    use cordex_model::gateway::Event;
    use serde_json::{Value, json};

    use super::*;

    fn message(id: u64, channel: u64) -> Value {
        let mut m: Value =
            serde_json::from_str(include_str!("../../model/tests/fixtures/message.json")).unwrap();
        m["id"] = json!(id.to_string());
        m["channel_id"] = json!(channel.to_string());
        m
    }

    /// Mock Discord: channel 100 has messages 1..=250, channel 101 answers
    /// 403. Honors `limit` and `before` like the real endpoint.
    fn client(calls: Arc<AtomicUsize>) -> Client {
        mock::client(move |req| {
            calls.fetch_add(1, Ordering::SeqCst);
            let path = req.uri().path().to_owned();
            if path.contains("/channels/101/") {
                return MockResponse::status(
                    cordex_http::StatusCode::FORBIDDEN,
                    r#"{"code": 50001, "message": "Missing Access"}"#,
                );
            }
            let query = req.uri().query().unwrap_or("");
            let param = |key: &str| {
                query
                    .split('&')
                    .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
                    .and_then(|v| v.parse::<u64>().ok())
            };
            let limit = param("limit").unwrap_or(50);
            let before = param("before").unwrap_or(251);
            let page: Vec<Value> = (1..before)
                .rev()
                .take(usize::try_from(limit).unwrap())
                .map(|id| message(id, 100))
                .collect();
            MockResponse::json(Value::Array(page).to_string())
        })
        .unwrap()
    }

    fn cache(size: usize) -> Arc<InMemoryCache> {
        Arc::new(InMemoryCache::new(
            CacheConfig::new()
                .resources(ResourceType::CHANNELS | ResourceType::MESSAGES)
                .parts(CacheParts::MESSAGE_CONTENT)
                .message_cache_size(size),
        ))
    }

    #[tokio::test]
    async fn pages_until_the_cache_size() {
        let calls = Arc::new(AtomicUsize::new(0));
        let http = client(Arc::clone(&calls));
        let cache = cache(150);
        let channel = Id::new(100).unwrap();
        assert_eq!(messages(&http, &cache, channel).await.unwrap(), 150);
        assert_eq!(calls.load(Ordering::SeqCst), 2, "100 + 50");
        let ids: Vec<u64> = cache
            .channel_messages(channel)
            .iter()
            .map(|m| m.id.get())
            .collect();
        assert_eq!(ids.first(), Some(&101), "oldest kept message");
        assert_eq!(ids.last(), Some(&250), "newest message");
        assert_eq!(&*cache.channel_messages(channel)[0].content, "Supa Hot");
    }

    #[tokio::test]
    async fn stops_when_history_ends_and_skips_without_messages_switch() {
        let calls = Arc::new(AtomicUsize::new(0));
        let http = client(Arc::clone(&calls));
        let big = cache(1000);
        assert_eq!(
            messages(&http, &big, Id::new(100).unwrap()).await.unwrap(),
            250
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "100 + 100 + 50 (short page ends)"
        );

        let off = Arc::new(InMemoryCache::new(CacheConfig::new()));
        assert_eq!(
            messages(&http, &off, Id::new(100).unwrap()).await.unwrap(),
            0
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "no request when messages are off"
        );
    }

    #[tokio::test]
    async fn guild_backfill_reports_and_skips_forbidden_channels() {
        let calls = Arc::new(AtomicUsize::new(0));
        let http = client(Arc::clone(&calls));
        let cache = cache(20);
        let channel = |id: u64, kind: u8| json!({"id": id.to_string(), "type": kind, "guild_id": "1", "name": "c"});
        for (id, kind) in [(100, 0), (101, 0), (102, 4)] {
            let raw =
                serde_json::value::RawValue::from_string(channel(id, kind).to_string()).unwrap();
            cache.update(&Event::from_dispatch("CHANNEL_CREATE", Some(&raw)).unwrap());
        }
        let report =
            guild_messages(&http, &cache, Id::new(1).unwrap(), BackfillOptions::new()).await;
        assert_eq!(report.channels, 1);
        assert_eq!(report.messages, 20);
        assert_eq!(
            report.skipped.len(),
            1,
            "403 channel reported, category ignored"
        );
        assert_eq!(report.skipped[0].0.get(), 101);
    }

    #[tokio::test]
    async fn live_messages_win_over_backfill() {
        let http = client(Arc::new(AtomicUsize::new(0)));
        let cache = cache(5);
        let channel = Id::new(100).unwrap();
        let mut live = message(250, 100);
        live["content"] = json!("live");
        let raw = serde_json::value::RawValue::from_string(live.to_string()).unwrap();
        cache.update(&Event::from_dispatch("MESSAGE_CREATE", Some(&raw)).unwrap());
        assert_eq!(messages(&http, &cache, channel).await.unwrap(), 4);
        let msgs = cache.channel_messages(channel);
        assert_eq!(msgs.len(), 5);
        assert_eq!(&*msgs.last().unwrap().content, "live");
    }
}
