//! Shard manager: automatic/manual ranges, identify queue, fan-in events
//! (PLAN.md §9).

use std::ops::Range;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;

use drs_model::Token;
use drs_model::gateway::presence::UpdatePresence;
use drs_model::gateway::{GatewayBot, Intents};
use futures_core::Stream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::connection::{Connector, WsConnector};
use crate::error::GatewayError;
use crate::event_flags::EventTypeFlags;
use crate::identify::{IdentifyQueue, LocalIdentifyQueue};
use crate::shard::{GatewayBotSource, Shard, ShardBuilder};
use crate::types::{
    Backpressure, CloseMode, DEFAULT_EVENT_CAPACITY, DEFAULT_LARGE_THRESHOLD,
    DEFAULT_MEMBER_TIMEOUT, ManagerEvent, ShardId, ShardStatus,
};

enum ShardSpec {
    Automatic,
    Range { range: Range<u32>, total: u32 },
}

/// Builder for [`ShardManager`].
pub struct ShardManagerBuilder<C: Connector = WsConnector> {
    token: Token,
    intents: Intents,
    spec: ShardSpec,
    connector: C,
    http: Option<drs_http::Client>,
    bot: Option<GatewayBot>,
    identify_queue: Option<Arc<dyn IdentifyQueue>>,
    event_types: EventTypeFlags,
    backpressure: Backpressure,
    event_capacity: usize,
    large_threshold: u8,
    presence: Option<UpdatePresence>,
    member_timeout: Duration,
    cancel: CancellationToken,
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
    event_types: EventTypeFlags,
    backpressure: Backpressure,
    event_capacity: usize,
    large_threshold: u8,
    presence: Option<UpdatePresence>,
    member_timeout: Duration,
    cancel: CancellationToken,
    events: mpsc::Sender<ManagerEvent>,
    total: u32,
}

impl ShardManagerBuilder<WsConnector> {
    /// Start a manager builder. Defaults to [`Self::automatic`].
    #[must_use]
    pub fn new(token: impl AsRef<str>, intents: Intents) -> Self {
        Self {
            token: Token::new(token.as_ref()),
            intents,
            spec: ShardSpec::Automatic,
            connector: WsConnector,
            http: None,
            bot: None,
            identify_queue: None,
            event_types: EventTypeFlags::all(),
            backpressure: Backpressure::Disconnect,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            large_threshold: DEFAULT_LARGE_THRESHOLD,
            presence: None,
            member_timeout: DEFAULT_MEMBER_TIMEOUT,
            cancel: CancellationToken::new(),
        }
    }
}

impl<C: Connector> ShardManagerBuilder<C> {
    /// Use the recommended shard count from `GET /gateway/bot`.
    #[must_use]
    pub fn automatic(mut self) -> Self {
        self.spec = ShardSpec::Automatic;
        self
    }

    /// Run `range` of `total` shards (multi-process clustering).
    #[must_use]
    pub fn shards(mut self, range: Range<u32>, total: u32) -> Self {
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
            event_types: self.event_types,
            backpressure: self.backpressure,
            event_capacity: self.event_capacity,
            large_threshold: self.large_threshold,
            presence: self.presence,
            member_timeout: self.member_timeout,
            cancel: self.cancel,
        }
    }

    /// REST client used for `GET /gateway/bot` when no payload is injected.
    #[must_use]
    pub fn http(mut self, client: drs_http::Client) -> Self {
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

    /// Initial presence sent with Identify.
    #[must_use]
    pub fn presence(mut self, presence: UpdatePresence) -> Self {
        self.presence = Some(presence);
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

    /// Fetch `/gateway/bot` if needed, create shards (not yet started).
    pub async fn build(self) -> Result<ShardManager<C>, GatewayError> {
        let bot = self.resolve_bot().await?;
        let (ids, total) = match &self.spec {
            ShardSpec::Automatic => {
                if bot.shards == 0 {
                    return Err(GatewayError::validation(
                        "GET /gateway/bot recommended 0 shards",
                    ));
                }
                ((0..bot.shards).collect::<Vec<_>>(), bot.shards)
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
                event_types: self.event_types,
                backpressure: self.backpressure,
                event_capacity: self.event_capacity,
                large_threshold: self.large_threshold,
                presence: self.presence,
                member_timeout: self.member_timeout,
                cancel: self.cancel.clone(),
                events: tx,
                total,
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
        let client = drs_http::Client::new(self.token.expose()).map_err(GatewayError::http)?;
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
            .member_timeout(self.member_timeout)
            .cancellation_token(self.cancel.child_token())
            .manager_events(self.events.clone());
        let shard = if let Some(presence) = self.presence.clone() {
            shard.presence(presence)
        } else {
            shard
        }
        .build();

        let pipe = shard.event_pipe();
        let tx = self.events.clone();
        tokio::spawn(async move {
            while let Some(item) = pipe.pop().await {
                let event = match item {
                    Ok(event) => ManagerEvent::Dispatch { shard: id, event },
                    Err(error) => ManagerEvent::Error { shard: id, error },
                };
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });
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
    /// Async so callers can `await` it like the rest of the manager API
    /// (PLAN.md §9.2).
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
            infos.push(shard.close(mode).await.ok().flatten());
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

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<C: Connector> Drop for ShardManager<C> {
    fn drop(&mut self) {
        self.inner.cancel.cancel();
    }
}
