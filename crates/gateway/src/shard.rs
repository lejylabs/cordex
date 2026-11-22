//! Public shard API.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use cordex_model::Token;
use cordex_model::gateway::Intents;
use cordex_model::gateway::command::{
    Command, RequestChannelInfo, RequestGuildMembers, RequestSoundboardSounds, UpdateVoiceState,
};
use cordex_model::gateway::event::Event;
use cordex_model::gateway::presence::UpdatePresence;
use cordex_model::guild::Member;
use cordex_model::id::Id;
use cordex_model::id::marker::GuildMarker;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::connection::{Connector, WsConnector};
use crate::error::GatewayError;
use crate::event_flags::EventTypeFlags;
use crate::identify::{IdentifyQueue, ImmediateIdentifyQueue};
use crate::intents_check::warn_missing_intents;
use crate::queue::EventPipe;
use crate::session::{self, Shared, UserCommand};
use crate::spawn::{CurrentRuntime, Spawner};
use crate::types::{
    Backpressure, CloseMode, DEFAULT_COMMAND_CAPACITY, DEFAULT_EVENT_CAPACITY, DEFAULT_GATEWAY_URL,
    DEFAULT_LARGE_THRESHOLD, DEFAULT_MEMBER_TIMEOUT, Latency, MAX_PAYLOAD_BYTES, ManagerEvent,
    ResumeInfo, ShardId, ShardState, ShardStatus,
};

/// Source of `GET /gateway/bot`.
pub trait GatewayBotSource: Send + Sync {
    /// Fetch recommended URL, shard count and session-start limit.
    fn gateway_bot(
        &self,
    ) -> impl Future<Output = Result<cordex_model::gateway::GatewayBot, GatewayError>> + Send;
}

impl GatewayBotSource for cordex_http::Client {
    async fn gateway_bot(&self) -> Result<cordex_model::gateway::GatewayBot, GatewayError> {
        cordex_http::Client::gateway_bot(self)
            .await
            .map_err(GatewayError::http)
    }
}

/// A single gateway connection with heartbeat, resume and event delivery.
pub struct Shard<C: Connector = WsConnector> {
    shared: Arc<Shared<C>>,
    commands: mpsc::Sender<UserCommand>,
    /// Dropping an owning handle cancels the shard task.
    owner: bool,
}

/// Builder for [`Shard`].
pub struct ShardBuilder<C: Connector = WsConnector> {
    token: Token,
    intents: Intents,
    shard: ShardId,
    gateway_url: Box<str>,
    connector: C,
    identify_queue: Arc<dyn IdentifyQueue>,
    event_types: EventTypeFlags,
    backpressure: Backpressure,
    event_capacity: usize,
    large_threshold: u8,
    presence: Option<UpdatePresence>,
    member_timeout: Duration,
    resume: Option<ResumeInfo>,
    cancel: CancellationToken,
    manager_tx: Option<mpsc::Sender<ManagerEvent>>,
    spawner: Arc<dyn Spawner>,
}

impl ShardBuilder<WsConnector> {
    /// Start a builder with a bot token and intents.
    pub fn new(token: impl AsRef<str>, intents: Intents) -> Self {
        Self {
            token: Token::new(token.as_ref()),
            intents,
            shard: ShardId::one(),
            gateway_url: DEFAULT_GATEWAY_URL.into(),
            connector: WsConnector::new(),
            identify_queue: Arc::new(ImmediateIdentifyQueue),
            event_types: EventTypeFlags::all(),
            backpressure: Backpressure::Disconnect,
            event_capacity: DEFAULT_EVENT_CAPACITY,
            large_threshold: DEFAULT_LARGE_THRESHOLD,
            presence: None,
            member_timeout: DEFAULT_MEMBER_TIMEOUT,
            resume: None,
            cancel: CancellationToken::new(),
            manager_tx: None,
            spawner: Arc::new(CurrentRuntime),
        }
    }

    /// Fill the gateway URL from `GET /gateway/bot`.
    pub async fn discover(
        token: impl AsRef<str>,
        intents: Intents,
        source: &impl GatewayBotSource,
    ) -> Result<Self, GatewayError> {
        let bot = source.gateway_bot().await?;
        Ok(Self::new(token, intents).gateway_url(bot.url))
    }
}

impl<C: Connector> ShardBuilder<C> {
    /// Shard number and total count.
    #[must_use]
    pub fn shard(mut self, id: ShardId) -> Self {
        self.shard = id;
        self
    }

    /// Gateway base URL (query string is added). Default
    /// `wss://gateway.discord.gg`.
    #[must_use]
    pub fn gateway_url(mut self, url: impl Into<Box<str>>) -> Self {
        self.gateway_url = url.into();
        self
    }

    /// Inject a connector (tests use [`crate::mock::MockConnector`]).
    #[must_use]
    pub fn connector<N: Connector>(self, connector: N) -> ShardBuilder<N> {
        ShardBuilder {
            token: self.token,
            intents: self.intents,
            shard: self.shard,
            gateway_url: self.gateway_url,
            connector,
            identify_queue: self.identify_queue,
            event_types: self.event_types,
            backpressure: self.backpressure,
            event_capacity: self.event_capacity,
            large_threshold: self.large_threshold,
            presence: self.presence,
            member_timeout: self.member_timeout,
            resume: self.resume,
            cancel: self.cancel,
            manager_tx: self.manager_tx,
            spawner: self.spawner,
        }
    }

    /// Where the shard task runs (default: [`CurrentRuntime`], the runtime
    /// calling [`Self::build`]). Use [`crate::RuntimeHandles`] for
    /// thread-per-core.
    #[must_use]
    pub fn spawn_with(mut self, spawner: Arc<dyn Spawner>) -> Self {
        self.spawner = spawner;
        self
    }

    /// Parent cancellation token (the manager uses a child token per shard).
    #[must_use]
    pub fn cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    pub(crate) fn manager_events(mut self, tx: mpsc::Sender<ManagerEvent>) -> Self {
        self.manager_tx = Some(tx);
        self
    }

    /// Identify queue (default: immediate). Resumes skip the queue.
    #[must_use]
    pub fn identify_queue(mut self, queue: Arc<dyn IdentifyQueue>) -> Self {
        self.identify_queue = queue;
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

    /// Event buffer capacity (default 1024).
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

    /// Resume an existing session (rolling deploys).
    #[must_use]
    pub fn resume(mut self, info: ResumeInfo) -> Self {
        self.resume = Some(info);
        self
    }

    /// Spawn the shard task on the configured [`Spawner`] and return the
    /// handle.
    ///
    /// Fails with [`GatewayError::NoRuntime`] when the default spawner is used
    /// outside a Tokio runtime.
    pub fn build(self) -> Result<Shard<C>, GatewayError> {
        if cfg!(debug_assertions) {
            warn_missing_intents(self.intents, self.event_types);
        }
        let events = Arc::new(EventPipe::new(self.event_capacity));
        let (tx, rx) = mpsc::channel(DEFAULT_COMMAND_CAPACITY);
        let shared = Arc::new(Shared {
            token: self.token,
            intents: self.intents,
            shard: self.shard,
            gateway_url: self.gateway_url,
            connector: self.connector,
            identify_queue: self.identify_queue,
            event_types: self.event_types,
            backpressure: self.backpressure,
            large_threshold: self.large_threshold,
            presence: self.presence,
            member_timeout: self.member_timeout,
            cancel: self.cancel,
            events,
            state: parking_lot::Mutex::new(ShardState::Disconnected),
            resume: parking_lot::Mutex::new(self.resume),
            latency: parking_lot::Mutex::new(Latency::new()),
            last_ack: parking_lot::Mutex::new(None),
            guild_count: std::sync::atomic::AtomicU32::new(0),
            reconnects: std::sync::atomic::AtomicU32::new(0),
            manager_tx: self.manager_tx,
        });
        let runner = Arc::clone(&shared);
        self.spawner.spawn(
            shared.shard,
            Box::pin(async move {
                session::run_forever(runner, rx).await;
            }),
        )?;
        Ok(Shard {
            shared,
            commands: tx,
            owner: true,
        })
    }
}

impl Shard<WsConnector> {
    /// Builder with token and intents.
    pub fn builder(token: impl AsRef<str>, intents: Intents) -> ShardBuilder<WsConnector> {
        ShardBuilder::new(token, intents)
    }
}

impl<C: Connector> Shard<C> {
    /// Next dispatch event, or `None` when the shard has stopped.
    pub async fn next_event(&self) -> Option<Result<Event, GatewayError>> {
        self.shared.events.pop().await
    }

    /// Observable connection state.
    #[must_use]
    pub fn state(&self) -> ShardState {
        self.shared.state()
    }

    /// Status snapshot (state, latency, guild count, reconnects).
    #[must_use]
    pub fn status(&self) -> ShardStatus {
        self.shared.status()
    }

    pub(crate) fn event_pipe(&self) -> Arc<EventPipe> {
        Arc::clone(&self.shared.events)
    }

    pub(crate) fn clone_handle(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            commands: self.commands.clone(),
            owner: false,
        }
    }

    /// Shard id.
    #[must_use]
    pub fn id(&self) -> ShardId {
        self.shared.shard
    }

    /// Heartbeat round-trip samples.
    #[must_use]
    pub fn latency(&self) -> Latency {
        session::lock(&self.shared.latency).clone()
    }

    /// Current resume data, if a session is established.
    #[must_use]
    pub fn resume_info(&self) -> Option<ResumeInfo> {
        session::lock(&self.shared.resume).clone()
    }

    /// Events dropped by [`Backpressure::DropOldest`] / `DropNewest`.
    #[must_use]
    pub fn dropped_events(&self) -> u64 {
        self.shared.events.dropped()
    }

    /// Events waiting in the buffer (tests).
    #[must_use]
    pub fn queued_events(&self) -> usize {
        self.shared.events.len()
    }

    /// Update presence (opcode 3).
    pub async fn update_presence(&self, presence: &UpdatePresence) -> Result<(), GatewayError> {
        self.send_command(presence).await
    }

    /// Voice state update payload (opcode 4; no voice connection).
    pub async fn update_voice_state(&self, update: &UpdateVoiceState) -> Result<(), GatewayError> {
        self.send_command(update).await
    }

    /// Request soundboard sounds (opcode 31).
    pub async fn request_soundboard_sounds(
        &self,
        request: &RequestSoundboardSounds<'_>,
    ) -> Result<(), GatewayError> {
        self.send_command(request).await
    }

    /// Request channel info (opcode 43).
    pub async fn request_channel_info(
        &self,
        request: &RequestChannelInfo<'_>,
    ) -> Result<(), GatewayError> {
        self.send_command(request).await
    }

    /// Request guild members and collect chunks (opcode 8).
    pub async fn request_guild_members(
        &self,
        mut request: RequestGuildMembers<'_>,
    ) -> Result<Vec<Member>, GatewayError> {
        if request.query == Some("")
            && request.limit == Some(0)
            && !self.shared.intents.contains(Intents::GUILD_MEMBERS)
        {
            return Err(GatewayError::validation(
                "requesting all members requires the GUILD_MEMBERS intent",
            ));
        }
        let generated = format!("{:032x}", fastrand::u128(..));
        let nonce_ref = request.nonce.unwrap_or(generated.as_str());
        request.nonce = Some(nonce_ref);
        let nonce: Box<str> = nonce_ref.into();
        let mut buf = Vec::new();
        request
            .write_json(&mut buf)
            .map_err(|source| GatewayError::Serialize { source })?;
        if buf.len() > MAX_PAYLOAD_BYTES {
            return Err(GatewayError::PayloadTooLarge);
        }
        let (done, rx) = oneshot::channel();
        self.commands
            .send(UserCommand::RequestMembers {
                nonce,
                bytes: buf,
                done,
            })
            .await
            .map_err(|_| GatewayError::Stopped)?;
        rx.await.map_err(|_| GatewayError::Stopped)?
    }

    /// Request every member of a guild (`query: ""`, `limit: 0`).
    pub async fn request_all_members(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> Result<Vec<Member>, GatewayError> {
        self.request_guild_members(RequestGuildMembers::all(guild_id))
            .await
    }

    /// Close the shard. Resumable close returns [`ResumeInfo`] when a session
    /// existed.
    pub async fn close(&self, mode: CloseMode) -> Result<Option<ResumeInfo>, GatewayError> {
        let (done, rx) = oneshot::channel();
        self.commands
            .send(UserCommand::Close { mode, done })
            .await
            .map_err(|_| GatewayError::Stopped)?;
        rx.await.map_err(|_| GatewayError::Stopped)
    }

    async fn send_command<T: Command>(&self, command: &T) -> Result<(), GatewayError> {
        let mut buf = Vec::new();
        command
            .write_json(&mut buf)
            .map_err(|source| GatewayError::Serialize { source })?;
        if buf.len() > MAX_PAYLOAD_BYTES {
            return Err(GatewayError::PayloadTooLarge);
        }
        let (done, rx) = oneshot::channel();
        self.commands
            .send(UserCommand::Payload { bytes: buf, done })
            .await
            .map_err(|_| GatewayError::Stopped)?;
        rx.await.map_err(|_| GatewayError::Stopped)?
    }
}

impl<C: Connector> Drop for Shard<C> {
    fn drop(&mut self) {
        if self.owner {
            self.shared.cancel.cancel();
        }
    }
}

impl<C: Connector> fmt::Debug for Shard<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shard")
            .field("token", &self.shared.token)
            .field("shard", &self.shared.shard)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
