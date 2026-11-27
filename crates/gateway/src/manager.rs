//! Shard manager: automatic/manual ranges, identify queue, fan-in events.

use std::ops::Range;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::Duration;

use cordex_model::Token;
use cordex_model::gateway::command::UpdateVoiceState;
use cordex_model::gateway::presence::UpdatePresence;
use cordex_model::gateway::{GatewayBot, Intents};
use cordex_model::id::Id;
use cordex_model::id::marker::GuildMarker;
use futures_core::Stream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::connection::{Connector, WsConnector};
use crate::error::GatewayError;
use crate::event_flags::EventTypeFlags;
use crate::identify::{IdentifyQueue, LocalIdentifyQueue};
use crate::session_store::SessionStore;
use crate::shard::{GatewayBotSource, Shard, ShardBuilder};
use crate::spawn::{CurrentRuntime, Spawner};
use crate::types::{
    Backpressure, CloseMode, DEFAULT_EVENT_CAPACITY, DEFAULT_LARGE_THRESHOLD,
    DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_MEMBER_TIMEOUT, ManagerEvent, ShardId, ShardStatus,
};

type PresenceFn = Arc<dyn Fn(u32, u32) -> UpdatePresence + Send + Sync>;

/// Compute the shard range for one process in a multi-process cluster
/// (e.g. a Kubernetes `StatefulSet`).
///
/// Distributes remainders fairly: when `total_shards` is not evenly divisible
/// by `cluster_count`, the first `total_shards % cluster_count` clusters each
/// receive one extra shard. Returns `None` if `cluster_count == 0`,
/// `cluster_id >= cluster_count`, or this cluster would receive zero shards.
#[must_use]
pub fn cluster_shard_range(
    cluster_id: u32,
    cluster_count: u32,
    total_shards: u32,
) -> Option<Range<u32>> {
    if cluster_count == 0 || cluster_id >= cluster_count || total_shards == 0 {
        return None;
    }
    let base = total_shards / cluster_count;
    let rem = total_shards % cluster_count;
    let start = cluster_id * base + cluster_id.min(rem);
    let len = base + u32::from(cluster_id < rem);
    if len == 0 {
        return None;
    }
    Some(start..start + len)
}

/// Discord's maximum guilds per shard (baseline for [`AutoShardingConfig::guilds_per_shard`]).
pub const DISCORD_MAX_GUILDS_PER_SHARD: u32 = 2_500;

/// Tuning knobs for automatic sharding ([`ShardManagerBuilder::auto_sharding`]).
///
/// Lets you override how Discord's recommended shard count from
/// `GET /gateway/bot` is translated into the actual shard count:
/// - [`guilds_per_shard`](Self::guilds_per_shard): target fewer guilds per
///   shard than Discord's 2,500 cap (e.g. `1_000` scales `recommended` up by
///   `ceil(recommended * 2500 / 1000)`).
/// - [`min_shards`](Self::min_shards) / [`max_shards`](Self::max_shards): clamp
///   the resulting shard count.
/// - [`round_up_to_multiple`](Self::round_up_to_multiple): align the shard
///   count to a multiple (e.g. `max_concurrency` or your cluster count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AutoShardingConfig {
    guilds_per_shard: Option<u32>,
    min_shards: Option<u32>,
    max_shards: Option<u32>,
    multiple: Option<u32>,
}

impl AutoShardingConfig {
    /// Default configuration (uses Discord's recommendation verbatim).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            guilds_per_shard: None,
            min_shards: None,
            max_shards: None,
            multiple: None,
        }
    }

    /// Target maximum guilds per shard (`1..=2500`).
    ///
    /// Scales Discord's recommended shard count proportionally against
    /// [`DISCORD_MAX_GUILDS_PER_SHARD`] (`2500`).
    #[must_use]
    pub const fn guilds_per_shard(mut self, target: u32) -> Self {
        let clamped = if target == 0 {
            1
        } else if target > DISCORD_MAX_GUILDS_PER_SHARD {
            DISCORD_MAX_GUILDS_PER_SHARD
        } else {
            target
        };
        self.guilds_per_shard = Some(clamped);
        self
    }

    /// Lower bound on the resolved shard count (clamped to `>= 1`).
    #[must_use]
    pub const fn min_shards(mut self, min: u32) -> Self {
        self.min_shards = Some(if min == 0 { 1 } else { min });
        self
    }

    /// Upper bound on the resolved shard count (clamped to `>= 1`).
    #[must_use]
    pub const fn max_shards(mut self, max: u32) -> Self {
        self.max_shards = Some(if max == 0 { 1 } else { max });
        self
    }

    /// Round the resolved shard count up to a multiple of `n` (e.g. `4` or `16`).
    #[must_use]
    pub const fn round_up_to_multiple(mut self, n: u32) -> Self {
        self.multiple = if n <= 1 { None } else { Some(n) };
        self
    }

    /// Compute the effective shard count from Discord's `recommended` count.
    #[must_use]
    pub fn resolve(&self, recommended: u32) -> u32 {
        let mut count = recommended.max(1);
        if let Some(target) = self.guilds_per_shard {
            let scaled = u64::from(count)
                .saturating_mul(u64::from(DISCORD_MAX_GUILDS_PER_SHARD))
                .div_ceil(u64::from(target));
            count = u32::try_from(scaled).unwrap_or(u32::MAX).max(1);
        }
        if let Some(min) = self.min_shards {
            count = count.max(min);
        }
        if let Some(m) = self.multiple {
            count = count.div_ceil(m).saturating_mul(m);
        }
        if let Some(max) = self.max_shards {
            count = count.min(max);
        }
        count.max(1)
    }
}

enum ShardSpec {
    Automatic(AutoShardingConfig),
    Range { range: Range<u32>, total: u32 },
    List { ids: Vec<u32>, total: u32 },
}

/// Builder for [`ShardManager`].
pub struct ShardManagerBuilder<C: Connector = WsConnector> {
    token: Token,
    intents: Intents,
    spec: ShardSpec,
    connector: C,
    http: Option<cordex_http::Client>,
    bot: Option<GatewayBot>,
    identify_queue: Option<Arc<dyn IdentifyQueue>>,
    session_store: Option<Arc<dyn SessionStore>>,
    event_types: EventTypeFlags,
    backpressure: Backpressure,
    event_capacity: usize,
    large_threshold: u8,
    max_message_bytes: usize,
    presence: Option<UpdatePresence>,
    presence_fn: Option<PresenceFn>,
    member_timeout: Duration,
    cancel: CancellationToken,
    spawner: Arc<dyn Spawner>,
}

/// Owns a set of shards, a shared identify queue and a fan-in event stream.
pub struct ShardManager<C: Connector = WsConnector> {
    inner: Arc<Inner<C>>,
    events: Option<mpsc::Receiver<ManagerEvent>>,
}

/// Fan-in of [`ManagerEvent`]s from every shard in a [`ShardManager`].
pub struct ManagerEvents {
    rx: mpsc::Receiver<ManagerEvent>,
}

struct Inner<C: Connector> {
    factory: ShardFactory<C>,
    shards: Mutex<Vec<Option<Shard<C>>>>,
    ids: Vec<u32>,
    total: u32,
    recommended: u32,
    remaining_at_start: u32,
    started: AtomicBool,
    cancel: CancellationToken,
}

struct ShardFactory<C: Connector> {
    token: Token,
    intents: Intents,
    gateway_url: Box<str>,
    connector: C,
    identify_queue: Arc<dyn IdentifyQueue>,
    session_store: Option<Arc<dyn SessionStore>>,
    event_types: EventTypeFlags,
    backpressure: Backpressure,
    event_capacity: usize,
    large_threshold: u8,
    max_message_bytes: usize,
    presence: Option<UpdatePresence>,
    presence_fn: Option<PresenceFn>,
    member_timeout: Duration,
    cancel: CancellationToken,
    events: mpsc::Sender<ManagerEvent>,
    total: u32,
    spawner: Arc<dyn Spawner>,
}

impl ShardManagerBuilder<WsConnector> {
    /// Start a manager builder. Defaults to [`Self::automatic`].
    #[must_use]
    pub fn new(token: impl AsRef<str>, intents: Intents) -> Self {
        Self {
            token: Token::new(token.as_ref()),
            intents,
            spec: ShardSpec::Automatic(AutoShardingConfig::new()),
            connector: WsConnector::new(),
            http: None,
            bot: None,
            identify_queue: None,
            session_store: None,
            event_types: EventTypeFlags::all(),
            backpressure: Backpressure::Disconnect,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            large_threshold: DEFAULT_LARGE_THRESHOLD,
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            presence: None,
            presence_fn: None,
            member_timeout: DEFAULT_MEMBER_TIMEOUT,
            cancel: CancellationToken::new(),
            spawner: Arc::new(CurrentRuntime),
        }
    }
}

impl<C: Connector> ShardManagerBuilder<C> {
    /// Use the recommended shard count from `GET /gateway/bot`.
    #[must_use]
    pub fn automatic(mut self) -> Self {
        self.spec = ShardSpec::Automatic(AutoShardingConfig::new());
        self
    }

    /// Use automatic sharding with custom preferences (target guilds per shard,
    /// min/max bounds, or multiple alignment).
    #[must_use]
    pub fn auto_sharding(mut self, config: AutoShardingConfig) -> Self {
        self.spec = ShardSpec::Automatic(config);
        self
    }

    /// Run `range` of `total` shards (multi-process clustering).
    #[must_use]
    pub fn shards(mut self, range: Range<u32>, total: u32) -> Self {
        self.spec = ShardSpec::Range { range, total };
        self
    }

    /// Run an arbitrary list of shard IDs out of `total` shards (e.g. strided
    /// sharding `[0, 2, 4, 6]` or taking over specific shards on a node).
    #[must_use]
    pub fn shard_ids(mut self, ids: impl IntoIterator<Item = u32>, total: u32) -> Self {
        self.spec = ShardSpec::List {
            ids: ids.into_iter().collect(),
            total,
        };
        self
    }

    /// Run the shard slice assigned to `cluster_id` out of `cluster_count`
    /// processes for `total` shards (computed via [`cluster_shard_range`]).
    #[must_use]
    pub fn cluster(mut self, cluster_id: u32, cluster_count: u32, total: u32) -> Self {
        let range = cluster_shard_range(cluster_id, cluster_count, total).unwrap_or(0..0);
        self.spec = ShardSpec::Range { range, total };
        self
    }

    /// Inject a connector (tests use [`crate::mock::MockConnector`]).
    #[must_use]
    pub fn connector<N: Connector>(self, connector: N) -> ShardManagerBuilder<N> {
        ShardManagerBuilder {
            token: self.token,
            intents: self.intents,
            spec: self.spec,
            connector,
            http: self.http,
            bot: self.bot,
            identify_queue: self.identify_queue,
            session_store: self.session_store,
            event_types: self.event_types,
            backpressure: self.backpressure,
            event_capacity: self.event_capacity,
            large_threshold: self.large_threshold,
            max_message_bytes: self.max_message_bytes,
            presence: self.presence,
            presence_fn: self.presence_fn,
            member_timeout: self.member_timeout,
            cancel: self.cancel,
            spawner: self.spawner,
        }
    }

    /// REST client used for `GET /gateway/bot` when no payload is injected.
    #[must_use]
    pub fn http(mut self, client: cordex_http::Client) -> Self {
        self.http = Some(client);
        self
    }

    /// Inject a `GET /gateway/bot` payload (tests).
    #[must_use]
    pub fn gateway_bot(mut self, bot: GatewayBot) -> Self {
        self.bot = Some(bot);
        self
    }

    /// Shared identify queue. Default: [`LocalIdentifyQueue`] from the bot
    /// payload.
    #[must_use]
    pub fn identify_queue(mut self, queue: Arc<dyn IdentifyQueue>) -> Self {
        self.identify_queue = Some(queue);
        self
    }

    /// Persistent session store for zero-downtime `RESUME` across restarts.
    ///
    /// When set, each shard loads its saved [`crate::ResumeInfo`] on spawn and
    /// [`ShardManager::shutdown`] persists active resume state on
    /// [`CloseMode::Resumable`].
    #[must_use]
    pub fn session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Which dispatch names are deserialized (default: all).
    #[must_use]
    pub fn event_types(mut self, flags: EventTypeFlags) -> Self {
        self.event_types = flags;
        self
    }

    /// Slow-consumer policy (default: [`Backpressure::Disconnect`]).
    #[must_use]
    pub fn backpressure(mut self, policy: Backpressure) -> Self {
        self.backpressure = policy;
        self
    }

    /// Per-shard event buffer capacity (default 1024).
    #[must_use]
    pub fn event_capacity(mut self, cap: usize) -> Self {
        self.event_capacity = cap.max(1);
        self
    }

    /// Identify `large_threshold` (50–250, default 50).
    #[must_use]
    pub fn large_threshold(mut self, threshold: u8) -> Self {
        self.large_threshold = threshold.clamp(50, 250);
        self
    }

    /// Cap on one decompressed gateway message (default
    /// [`DEFAULT_MAX_MESSAGE_BYTES`], 16 MiB). A larger message ends the
    /// connection, which then reconnects. Raise it only for very large
    /// guilds whose `GUILD_CREATE` exceeds the default.
    #[must_use]
    pub fn max_message_bytes(mut self, bytes: usize) -> Self {
        self.max_message_bytes = bytes.max(1);
        self
    }

    /// Initial presence sent with Identify.
    #[must_use]
    pub fn presence(mut self, presence: UpdatePresence) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Per-shard initial presence builder `(shard_id, total_shards) -> UpdatePresence`.
    ///
    /// Overrides [`presence`](Self::presence) when set.
    #[must_use]
    pub fn presence_for_shard<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, u32) -> UpdatePresence + Send + Sync + 'static,
    {
        self.presence_fn = Some(Arc::new(f));
        self
    }

    /// Timeout for collecting `GUILD_MEMBERS_CHUNK` (default 30 s).
    #[must_use]
    pub fn member_timeout(mut self, timeout: Duration) -> Self {
        self.member_timeout = timeout;
        self
    }

    /// Cancellation token for the whole manager.
    #[must_use]
    pub fn cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    /// Where shard tasks run (default: [`CurrentRuntime`], the runtime calling
    /// [`ShardManager::start`]). Pass [`crate::RuntimeHandles`] to spread shards
    /// over several `current_thread` runtimes.
    #[must_use]
    pub fn spawn_with(mut self, spawner: Arc<dyn Spawner>) -> Self {
        self.spawner = spawner;
        self
    }

    /// Fetch `/gateway/bot` if needed, create shards (not yet started).
    pub async fn build(self) -> Result<ShardManager<C>, GatewayError> {
        let bot = self.resolve_bot().await?;
        let (ids, total) = match &self.spec {
            ShardSpec::Automatic(cfg) => {
                if bot.shards == 0 {
                    return Err(GatewayError::validation(
                        "GET /gateway/bot recommended 0 shards",
                    ));
                }
                let count = cfg.resolve(bot.shards);
                ((0..count).collect::<Vec<_>>(), count)
            }
            ShardSpec::Range { range, total } => {
                if *total == 0 || range.end > *total || range.start >= range.end {
                    return Err(GatewayError::InvalidShardId {
                        id: range.start,
                        total: *total,
                    });
                }
                ((range.start..range.end).collect::<Vec<_>>(), *total)
            }
            ShardSpec::List { ids, total } => {
                if *total == 0 || ids.is_empty() {
                    return Err(GatewayError::InvalidShardId {
                        id: 0,
                        total: *total,
                    });
                }
                let mut seen = std::collections::HashSet::with_capacity(ids.len());
                for &id in ids {
                    if id >= *total {
                        return Err(GatewayError::InvalidShardId { id, total: *total });
                    }
                    if !seen.insert(id) {
                        return Err(GatewayError::validation(format!(
                            "duplicate shard id {id} in shard_ids"
                        )));
                    }
                }
                (ids.clone(), *total)
            }
        };
        if ids.is_empty() {
            return Err(GatewayError::validation("shard manager range is empty"));
        }

        let queue = self.identify_queue.clone().unwrap_or_else(|| {
            Arc::new(
                LocalIdentifyQueue::new(bot.session_start_limit.max_concurrency)
                    .with_session_limit(bot.session_start_limit),
            )
        });

        let cap = self.event_capacity.saturating_mul(ids.len().max(1));
        let (tx, rx) = mpsc::channel(cap.max(1));
        let inner = Arc::new(Inner {
            factory: ShardFactory {
                token: self.token,
                intents: self.intents,
                gateway_url: bot.url.clone(),
                connector: self.connector,
                identify_queue: queue,
                session_store: self.session_store,
                event_types: self.event_types,
                backpressure: self.backpressure,
                event_capacity: self.event_capacity,
                large_threshold: self.large_threshold,
                max_message_bytes: self.max_message_bytes,
                presence: self.presence,
                presence_fn: self.presence_fn,
                member_timeout: self.member_timeout,
                cancel: self.cancel.clone(),
                events: tx,
                total,
                spawner: self.spawner,
            },
            shards: Mutex::new((0..ids.len()).map(|_| None).collect()),
            ids,
            total,
            recommended: bot.shards,
            remaining_at_start: bot.session_start_limit.remaining,
            started: AtomicBool::new(false),
            cancel: self.cancel,
        });

        Ok(ShardManager {
            inner,
            events: Some(rx),
        })
    }

    async fn resolve_bot(&self) -> Result<GatewayBot, GatewayError> {
        if let Some(bot) = self.bot.clone() {
            return Ok(bot);
        }
        if let Some(http) = &self.http {
            return http.gateway_bot().await.map_err(GatewayError::http);
        }
        let client = cordex_http::Client::new(self.token.expose()).map_err(GatewayError::http)?;
        GatewayBotSource::gateway_bot(&client).await
    }
}

impl<C: Connector> ShardFactory<C> {
    fn spawn(&self, number: u32) -> Result<Shard<C>, GatewayError> {
        let id = ShardId::new(number, self.total)?;
        let shard = ShardBuilder::new(self.token.expose(), self.intents)
            .shard(id)
            .gateway_url(self.gateway_url.clone())
            .connector(self.connector.clone())
            .identify_queue(Arc::clone(&self.identify_queue))
            .event_types(self.event_types)
            .backpressure(self.backpressure)
            .event_capacity(self.event_capacity)
            .large_threshold(self.large_threshold)
            .max_message_bytes(self.max_message_bytes)
            .member_timeout(self.member_timeout)
            .cancellation_token(self.cancel.child_token())
            .manager_events(self.events.clone())
            .spawn_with(Arc::clone(&self.spawner));
        let presence = self
            .presence_fn
            .as_ref()
            .map(|f| f(number, self.total))
            .or_else(|| self.presence.clone());
        let shard = if let Some(presence) = presence {
            shard.presence(presence)
        } else {
            shard
        };
        let shard = if let Some(info) = self
            .session_store
            .as_ref()
            .and_then(|store| store.load(number))
        {
            shard.resume(info)
        } else {
            shard
        }
        .build()?;

        let pipe = shard.event_pipe();
        let tx = self.events.clone();
        // The fan-in forwarder runs on the same runtime as its shard.
        self.spawner.spawn(
            id,
            Box::pin(async move {
                while let Some(item) = pipe.pop().await {
                    let event = match item {
                        Ok(event) => ManagerEvent::Dispatch { shard: id, event },
                        Err(error) => ManagerEvent::Error { shard: id, error },
                    };
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }),
        )?;
        Ok(shard)
    }
}

impl ShardManager<WsConnector> {
    /// Builder with token and intents (automatic sharding by default).
    #[must_use]
    pub fn builder(token: impl AsRef<str>, intents: Intents) -> ShardManagerBuilder<WsConnector> {
        ShardManagerBuilder::new(token, intents)
    }
}

impl<C: Connector> ShardManager<C> {
    /// Spawn every shard in the configured range.
    ///
    /// When `session_start_limit.remaining` is lower than the shard count, the
    /// extra shards still start; the identify queue waits for `reset_after`.
    /// Async so callers can `await` it like the rest of the manager API.
    #[allow(clippy::unused_async, reason = "public API is `.start().await`")]
    #[allow(
        clippy::unused_async_trait_impl,
        reason = "public API is `.start().await`"
    )]
    pub async fn start(&self) -> Result<(), GatewayError> {
        if self.inner.started.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let running = u32::try_from(self.inner.ids.len()).unwrap_or(u32::MAX);
        if self.inner.recommended > running {
            tracing::warn!(
                running,
                recommended = self.inner.recommended,
                "GET /gateway/bot recommends more shards than this process is running"
            );
            let _ = self
                .inner
                .factory
                .events
                .try_send(ManagerEvent::RecommendedShardCount {
                    running,
                    recommended: self.inner.recommended,
                });
        }
        if self.inner.remaining_at_start < running {
            tracing::warn!(
                remaining = self.inner.remaining_at_start,
                starting = running,
                "session start remaining is below the shard count; extra identifies wait for reset_after"
            );
        }

        let mut created = Vec::with_capacity(self.inner.ids.len());
        for number in &self.inner.ids {
            created.push(Some(self.inner.factory.spawn(*number)?));
        }
        *lock(&self.inner.shards) = created;
        Ok(())
    }

    /// Take the fan-in stream. Call once; later calls yield an already-closed
    /// stream.
    pub fn events(&mut self) -> ManagerEvents {
        if let Some(rx) = self.events.take() {
            ManagerEvents { rx }
        } else {
            let (tx, rx) = mpsc::channel(1);
            drop(tx);
            ManagerEvents { rx }
        }
    }

    /// Status of every started shard.
    #[must_use]
    pub fn status(&self) -> Vec<ShardStatus> {
        lock(&self.inner.shards)
            .iter()
            .filter_map(|slot| slot.as_ref().map(Shard::status))
            .collect()
    }

    /// Non-owning handle to one shard (commands and status). Event delivery
    /// stays on the manager fan-in stream.
    #[must_use]
    pub fn shard(&self, number: u32) -> Option<Shard<C>> {
        lock(&self.inner.shards)
            .iter()
            .find(|slot| slot.as_ref().is_some_and(|s| s.id().number() == number))
            .and_then(|slot| slot.as_ref().map(Shard::clone_handle))
    }

    /// Non-owning handle to the shard responsible for `guild_id`, if this
    /// manager owns that shard.
    #[must_use]
    pub fn shard_for_guild(&self, guild_id: Id<GuildMarker>) -> Option<Shard<C>> {
        let id = ShardId::for_guild(guild_id, self.inner.total).ok()?;
        self.shard(id.number())
    }

    /// Send a voice state update (opcode 4) to the shard responsible for
    /// `update.guild_id`.
    pub async fn update_voice_state(&self, update: &UpdateVoiceState) -> Result<(), GatewayError> {
        let id = ShardId::for_guild(update.guild_id, self.inner.total)?;
        let shard = self
            .shard(id.number())
            .ok_or(GatewayError::InvalidShardId {
                id: id.number(),
                total: self.inner.total,
            })?;
        shard.update_voice_state(update).await
    }

    /// Recommended shard count from `GET /gateway/bot`.
    #[must_use]
    pub fn recommended_shards(&self) -> u32 {
        self.inner.recommended
    }

    /// Total shard count this process is configured with.
    #[must_use]
    pub fn total_shards(&self) -> u32 {
        self.inner.total
    }

    /// Close one shard and start it again (new identify, no resume).
    pub async fn restart_shard(&self, number: u32) -> Result<(), GatewayError> {
        let idx = self.inner.ids.iter().position(|id| *id == number).ok_or(
            GatewayError::InvalidShardId {
                id: number,
                total: self.inner.total,
            },
        )?;
        let old = {
            let mut shards = lock(&self.inner.shards);
            shards.get_mut(idx).and_then(Option::take)
        };
        if let Some(old) = old {
            let _ = old.close(CloseMode::Terminate).await;
            drop(old);
        }
        let new = self.inner.factory.spawn(number)?;
        lock(&self.inner.shards)[idx] = Some(new);
        Ok(())
    }

    /// Restart every shard in order, waiting for each close.
    pub async fn restart_all(&self) -> Result<(), GatewayError> {
        let ids = self.inner.ids.clone();
        for number in ids {
            self.restart_shard(number).await?;
        }
        Ok(())
    }

    /// Close every shard. Resumable close returns per-shard [`crate::ResumeInfo`].
    pub async fn shutdown(&self, mode: CloseMode) -> Vec<Option<crate::ResumeInfo>> {
        self.inner.cancel.cancel();
        let shards: Vec<Shard<C>> = lock(&self.inner.shards)
            .iter_mut()
            .filter_map(Option::take)
            .collect();
        let mut infos = Vec::with_capacity(shards.len());
        for shard in shards {
            let number = shard.id().number();
            let info = shard.close(mode).await.ok().flatten();
            if let Some(store) = &self.inner.factory.session_store {
                match (&info, mode) {
                    (Some(resume), CloseMode::Resumable) => {
                        if let Err(err) = store.save(number, resume) {
                            tracing::warn!(shard = number, error = %err, "failed to save shard session");
                        }
                    }
                    (_, CloseMode::Terminate) => {
                        let _ = store.clear(number);
                    }
                    _ => {}
                }
            }
            infos.push(info);
        }
        self.inner.started.store(false, Ordering::SeqCst);
        infos
    }
}

impl ManagerEvents {
    /// Next manager event, or `None` when the manager has shut down.
    pub async fn recv(&mut self) -> Option<ManagerEvent> {
        self.rx.recv().await
    }
}

impl Stream for ManagerEvents {
    type Item = ManagerEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock()
}

impl<C: Connector> Drop for ShardManager<C> {
    fn drop(&mut self) {
        self.inner.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_shard_range_distributes_remainders_fairly() {
        // 10 shards across 3 clusters -> [0..4, 4..7, 7..10]
        assert_eq!(cluster_shard_range(0, 3, 10), Some(0..4));
        assert_eq!(cluster_shard_range(1, 3, 10), Some(4..7));
        assert_eq!(cluster_shard_range(2, 3, 10), Some(7..10));
        assert_eq!(cluster_shard_range(3, 3, 10), None);
        assert_eq!(cluster_shard_range(0, 0, 10), None);
        assert_eq!(cluster_shard_range(0, 3, 0), None);
        // More clusters than shards: extra clusters get None
        assert_eq!(cluster_shard_range(0, 4, 2), Some(0..1));
        assert_eq!(cluster_shard_range(1, 4, 2), Some(1..2));
        assert_eq!(cluster_shard_range(2, 4, 2), None);
    }

    #[test]
    fn auto_sharding_config_resolves_preferences() {
        // Default keeps Discord's recommendation
        assert_eq!(AutoShardingConfig::new().resolve(4), 4);
        // 1000 guilds/shard scales 2 recommended shards -> ceil(2 * 2500 / 1000) = 5
        assert_eq!(
            AutoShardingConfig::new().guilds_per_shard(1_000).resolve(2),
            5
        );
        // Round up to multiple of 4 -> 8
        assert_eq!(
            AutoShardingConfig::new()
                .guilds_per_shard(1_000)
                .round_up_to_multiple(4)
                .resolve(2),
            8
        );
        // Min and max clamps
        assert_eq!(AutoShardingConfig::new().min_shards(4).resolve(1), 4);
        assert_eq!(AutoShardingConfig::new().max_shards(6).resolve(10), 6);
    }
}
