//! One connection lifetime: hello, identify/resume, heartbeat, dispatch.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use bytes::Bytes;
use cordex_model::gateway::command::{Command, ConnectionProperties, Heartbeat, Identify, Resume};
use cordex_model::gateway::event::{Event, GuildMembersChunk};
use cordex_model::gateway::{CloseAction, CloseCode, Envelope, Hello, Intents, OpCode};
use cordex_model::guild::Member;
use parking_lot::{Mutex, MutexGuard};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, timeout};
use tokio_util::sync::CancellationToken;

use crate::connection::{Connection, Connector, Incoming, Outgoing};
use crate::error::GatewayError;
use crate::event_flags::EventTypeFlags;
use crate::identify::IdentifyQueue;
use crate::outbound::OutboundLimiter;
use crate::queue::{EventPipe, Push};
use crate::types::{
    Backpressure, CLOSE_TIMEOUT, CloseMode, HELLO_TIMEOUT, LIBRARY_NAME, Latency,
    MAX_PAYLOAD_BYTES, ManagerEvent, READY_TIMEOUT, ResumeInfo, STABLE_ACTIVE, ShardId, ShardState,
    ShardStatus, with_gateway_query,
};
use crate::zlib::ZlibStream;
use cordex_model::Token;

pub(crate) enum UserCommand {
    Payload {
        bytes: Vec<u8>,
        done: oneshot::Sender<Result<(), GatewayError>>,
    },
    RequestMembers {
        nonce: Box<str>,
        bytes: Vec<u8>,
        done: oneshot::Sender<Result<Vec<Member>, GatewayError>>,
    },
    Close {
        mode: CloseMode,
        done: oneshot::Sender<Option<ResumeInfo>>,
    },
}

pub(crate) struct Shared<C> {
    pub token: Token,
    pub intents: Intents,
    pub shard: ShardId,
    pub gateway_url: Box<str>,
    pub connector: C,
    pub identify_queue: Arc<dyn IdentifyQueue>,
    pub event_types: EventTypeFlags,
    pub backpressure: Backpressure,
    pub large_threshold: u8,
    pub presence: Option<cordex_model::gateway::presence::UpdatePresence>,
    pub member_timeout: Duration,
    pub cancel: CancellationToken,
    pub events: Arc<EventPipe>,
    pub state: Mutex<ShardState>,
    pub resume: Mutex<Option<ResumeInfo>>,
    pub latency: Mutex<Latency>,
    pub last_ack: Mutex<Option<Instant>>,
    pub guild_count: AtomicU32,
    pub reconnects: AtomicU32,
    pub manager_tx: Option<tokio::sync::mpsc::Sender<ManagerEvent>>,
}

impl<C> Shared<C> {
    pub(crate) fn set_state(&self, new: ShardState) {
        let mut slot = lock(&self.state);
        let old = *slot;
        if old != new {
            tracing::info!(shard = self.shard.number(), ?old, ?new, "gateway state");
            *slot = new;
            drop(slot);
            self.emit(ManagerEvent::StateChanged {
                shard: self.shard,
                old,
                new,
            });
        }
    }

    pub(crate) fn state(&self) -> ShardState {
        *lock(&self.state)
    }

    pub(crate) fn status(&self) -> ShardStatus {
        let last_heartbeat_ack = lock(&self.last_ack).map(|at| at.elapsed());
        ShardStatus {
            id: self.shard,
            state: self.state(),
            latency: lock(&self.latency).clone(),
            last_heartbeat_ack,
            guild_count: self.guild_count.load(Ordering::Relaxed),
            reconnect_count: self.reconnects.load(Ordering::Relaxed),
        }
    }

    fn emit(&self, event: ManagerEvent) {
        if let Some(tx) = &self.manager_tx {
            let _ = tx.try_send(event);
        }
    }

    fn resume_info(&self) -> Option<ResumeInfo> {
        lock(&self.resume).clone()
    }

    fn store_resume(&self, info: ResumeInfo) {
        *lock(&self.resume) = Some(info);
    }

    fn clear_resume(&self) {
        *lock(&self.resume) = None;
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock()
}

enum SessionEnd {
    Reconnect {
        use_resume: bool,
        apply_backoff: bool,
        stable: bool,
    },
    Fatal(GatewayError),
    Shutdown {
        resume: Option<ResumeInfo>,
    },
}

struct PendingMembers {
    members: Vec<Member>,
    done: oneshot::Sender<Result<Vec<Member>, GatewayError>>,
    deadline: Instant,
}

struct Session<'a, C: Connector, Conn> {
    shared: &'a Shared<C>,
    conn: &'a mut Conn,
    zlib: ZlibStream,
    outbound: OutboundLimiter,
    seq: Option<u64>,
    next_heartbeat: Option<Instant>,
    heartbeat_interval: Option<Duration>,
    awaiting_ack: bool,
    heartbeat_sent_at: Option<Instant>,
    phase_deadline: Option<Instant>,
    try_resume: bool,
    pending_event: Option<Event>,
    identify_at: Option<Instant>,
    user_q: VecDeque<UserCommand>,
    members: HashMap<Box<str>, PendingMembers>,
    json_buf: Vec<u8>,
    active_since: Option<Instant>,
}

pub(crate) async fn run_forever<C: Connector>(
    shared: Arc<Shared<C>>,
    mut commands: mpsc::Receiver<UserCommand>,
) {
    let mut failures: u32 = 0;
    let mut panics: VecDeque<Instant> = VecDeque::new();
    let mut try_resume = shared.resume_info().is_some();

    loop {
        if shared.cancel.is_cancelled() {
            shared.set_state(ShardState::Disconnected);
            shared.events.close();
            return;
        }

        let (fwd_tx, fwd_rx) = mpsc::unbounded_channel();
        let shared_task = Arc::clone(&shared);
        // One task per connection lifetime is the panic guard:
        // a panic surfaces as a `JoinError` instead of unwinding through this
        // loop. It runs on this task's runtime, i.e. the one the `Spawner`
        // chose, so no runtime is picked behind the caller's back.
        let mut handle =
            tokio::spawn(async move { run_session(shared_task, fwd_rx, try_resume).await });

        let join = loop {
            tokio::select! {
                biased;
                join = &mut handle => break join,
                cmd = commands.recv() => {
                    match cmd {
                        None => {
                            shared.cancel.cancel();
                            drop(fwd_tx);
                            break handle.await;
                        }
                        Some(cmd) => {
                            if fwd_tx.send(cmd).is_err() {
                                break handle.await;
                            }
                        }
                    }
                }
            }
        };

        match handle_join(join, &shared, &mut try_resume, &mut failures, &mut panics).await {
            Loop::Stop => return,
            Loop::Continue => {}
        }
    }
}

enum Loop {
    Stop,
    Continue,
}

async fn handle_join<C: Connector>(
    join: Result<SessionEnd, tokio::task::JoinError>,
    shared: &Arc<Shared<C>>,
    try_resume: &mut bool,
    failures: &mut u32,
    panics: &mut VecDeque<Instant>,
) -> Loop {
    match join {
        Ok(SessionEnd::Shutdown { resume }) => {
            if resume.is_none() {
                shared.clear_resume();
            }
            shared.set_state(ShardState::Disconnected);
            shared.events.close();
            Loop::Stop
        }
        Ok(SessionEnd::Fatal(err)) => {
            tracing::error!(shard = shared.shard.number(), %err, "fatal gateway close");
            shared.set_state(ShardState::Disconnected);
            shared.events.push_error(err);
            Loop::Stop
        }
        Ok(SessionEnd::Reconnect {
            use_resume,
            apply_backoff,
            stable,
        }) => {
            *try_resume = use_resume && shared.resume_info().is_some();
            if stable {
                *failures = 0;
            }
            shared.reconnects.fetch_add(1, Ordering::Relaxed);
            if apply_backoff {
                *failures = failures.saturating_add(1);
                shared.set_state(ShardState::Reconnecting);
                let wait = backoff_delay(failures.saturating_sub(1));
                tracing::warn!(
                    shard = shared.shard.number(),
                    ?wait,
                    failures = *failures,
                    try_resume = *try_resume,
                    "reconnecting"
                );
                tokio::select! {
                    () = tokio::time::sleep(wait) => Loop::Continue,
                    () = shared.cancel.cancelled() => {
                        shared.set_state(ShardState::Disconnected);
                        shared.events.close();
                        Loop::Stop
                    }
                }
            } else {
                *failures = 0;
                Loop::Continue
            }
        }
        Err(join) if join.is_cancelled() => {
            shared.set_state(ShardState::Disconnected);
            shared.events.close();
            Loop::Stop
        }
        Err(join) => {
            let msg = panic_message(join);
            tracing::error!(shard = shared.shard.number(), panic = %msg, "shard panicked");
            let now = Instant::now();
            panics.push_back(now);
            while panics
                .front()
                .is_some_and(|t| now.duration_since(*t) > crate::types::PANIC_WINDOW)
            {
                panics.pop_front();
            }
            if panics.len() > crate::types::PANIC_LIMIT as usize {
                tracing::error!(
                    shard = shared.shard.number(),
                    "too many panics; stopping the shard"
                );
                shared.set_state(ShardState::Disconnected);
                shared.events.push_error(GatewayError::PanicLimit(msg));
                return Loop::Stop;
            }
            shared.events.push_soft_error(GatewayError::Panic(msg));
            *try_resume = shared.resume_info().is_some();
            *failures = failures.saturating_add(1);
            shared.reconnects.fetch_add(1, Ordering::Relaxed);
            shared.set_state(ShardState::Reconnecting);
            tokio::select! {
                () = tokio::time::sleep(backoff_delay(failures.saturating_sub(1))) => Loop::Continue,
                () = shared.cancel.cancelled() => {
                    shared.set_state(ShardState::Disconnected);
                    shared.events.close();
                    Loop::Stop
                }
            }
        }
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    let exp = attempt.min(6);
    let secs = (1u64 << exp).min(60);
    let percent = 80 + fastrand::u64(0..41);
    Duration::from_millis(secs.saturating_mul(10) * percent)
}

fn panic_message(err: tokio::task::JoinError) -> Box<str> {
    let payload = err.into_panic();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).into()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone().into_boxed_str()
    } else {
        "panic".into()
    }
}

async fn run_session<C: Connector>(
    shared: Arc<Shared<C>>,
    mut commands: mpsc::UnboundedReceiver<UserCommand>,
    try_resume: bool,
) -> SessionEnd {
    shared.set_state(ShardState::Connecting);
    let url = if try_resume {
        shared.resume_info().map_or_else(
            || with_gateway_query(&shared.gateway_url),
            |info| with_gateway_query(&info.resume_gateway_url),
        )
    } else {
        with_gateway_query(&shared.gateway_url)
    };
    tracing::debug!(shard = shared.shard.number(), %url, try_resume, "connecting");
    let mut conn = match shared.connector.connect(&url).await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!(shard = shared.shard.number(), %err, "connect failed");
            return SessionEnd::Reconnect {
                use_resume: try_resume,
                apply_backoff: true,
                stable: false,
            };
        }
    };

    shared.set_state(ShardState::AwaitingHello);
    let mut sess = Session {
        shared: &*shared,
        conn: &mut conn,
        zlib: ZlibStream::new(),
        outbound: OutboundLimiter::new(),
        seq: shared.resume_info().map(|i| i.seq),
        next_heartbeat: None,
        heartbeat_interval: None,
        awaiting_ack: false,
        heartbeat_sent_at: None,
        phase_deadline: Some(Instant::now() + HELLO_TIMEOUT),
        try_resume,
        pending_event: None,
        identify_at: None,
        user_q: VecDeque::new(),
        members: HashMap::new(),
        json_buf: Vec::with_capacity(256),
        active_since: None,
    };

    sess.run(&mut commands).await
}

impl<C: Connector, Conn: Connection> Session<'_, C, Conn> {
    async fn run(&mut self, commands: &mut mpsc::UnboundedReceiver<UserCommand>) -> SessionEnd {
        loop {
            self.expire_member_requests();
            if self.shared.cancel.is_cancelled() {
                return self.shutdown(CloseMode::Terminate, None).await;
            }
            self.flush_block_event();
            let now = Instant::now();
            let user_ready_at = if self.user_q.is_empty() {
                None
            } else {
                Some(now + self.outbound.user_wait(now))
            };
            let wait_space = self.pending_event.is_some();

            tokio::select! {
                biased;
                () = self.shared.cancel.cancelled() => {
                    return self.shutdown(CloseMode::Terminate, None).await;
                }
                () = self.shared.events.wait_space(), if wait_space => {
                    self.flush_block_event();
                }
                cmd = commands.recv() => {
                    match cmd {
                        None => return self.shutdown(CloseMode::Terminate, None).await,
                        Some(UserCommand::Close { mode, done }) => {
                            return self.shutdown(mode, Some(done)).await;
                        }
                        Some(other) => self.user_q.push_back(other),
                    }
                }
                frame = self.conn.recv() => {
                    match self.on_frame(frame).await {
                        None => {}
                        Some(end) => return end,
                    }
                }
                () = sleep_until_opt(self.next_heartbeat) => {
                    match self.on_heartbeat().await {
                        None => {}
                        Some(end) => return end,
                    }
                }
                () = sleep_until_opt(self.phase_deadline) => {
                    tracing::warn!(shard = self.shared.shard.number(), "hello/ready timeout");
                    return self.reconnect_close(true, true).await;
                }
                () = sleep_until_opt(user_ready_at) => {
                    if let Some(end) = self.flush_user().await {
                        return end;
                    }
                }
                () = sleep_until_opt(self.identify_at) => {
                    self.identify_at = None;
                    if let Some(end) = self.send_identify().await {
                        return end;
                    }
                }
            }
        }
    }

    fn flush_block_event(&mut self) {
        if self.shared.backpressure != Backpressure::Block {
            return;
        }
        if let Some(event) = self.pending_event.take() {
            match self
                .shared
                .events
                .push_event(event, self.shared.backpressure)
            {
                Push::Ok => {}
                Push::Full(event) => self.pending_event = Some(event),
                Push::Disconnect => {
                    self.phase_deadline = Some(Instant::now());
                    self.try_resume = true;
                }
            }
        }
    }

    async fn on_frame(
        &mut self,
        frame: Result<Option<Incoming>, GatewayError>,
    ) -> Option<SessionEnd> {
        match frame {
            Err(err) => {
                tracing::warn!(shard = self.shared.shard.number(), %err, "recv failed");
                Some(self.reconnect_end(self.shared.resume_info().is_some(), true))
            }
            Ok(None) => {
                tracing::warn!(shard = self.shared.shard.number(), "tcp drop");
                Some(self.reconnect_end(self.shared.resume_info().is_some(), true))
            }
            Ok(Some(Incoming::Close { code, reason })) => Some(self.on_close(code, &reason)),
            Ok(Some(Incoming::Text(text))) => self.on_json(&text).await,
            Ok(Some(Incoming::Binary(bytes))) => match self.zlib.push(&bytes) {
                Err(err) => {
                    tracing::error!(shard = self.shared.shard.number(), %err, "zlib");
                    Some(self.reconnect_close_now(true, true).await)
                }
                Ok(messages) => {
                    for json in messages {
                        if let Some(end) = self.on_json(&json).await {
                            return Some(end);
                        }
                    }
                    None
                }
            },
        }
    }

    fn on_close(&mut self, code: Option<u16>, reason: &str) -> SessionEnd {
        let code = CloseCode::from(code.unwrap_or(1006));
        tracing::info!(
            shard = self.shared.shard.number(),
            ?code,
            reason,
            "gateway closed"
        );
        match code.action() {
            CloseAction::Fatal => {
                let reason = if code == CloseCode::DisallowedIntents {
                    format!(
                        "disallowed intents (privileged: {:?})",
                        self.shared.intents.privileged()
                    )
                    .into_boxed_str()
                } else {
                    reason.into()
                };
                SessionEnd::Fatal(GatewayError::Fatal { code, reason })
            }
            CloseAction::Reidentify => {
                self.shared.clear_resume();
                self.reconnect_end(false, true)
            }
            CloseAction::Resume => self.reconnect_end(true, true),
        }
    }

    async fn on_json(&mut self, json: &str) -> Option<SessionEnd> {
        let envelope = match Envelope::parse(json) {
            Ok(env) => env,
            Err(err) => {
                tracing::debug!(shard = self.shared.shard.number(), %err, "malformed gateway json");
                return None;
            }
        };
        match envelope.op {
            OpCode::Dispatch => self.on_dispatch(&envelope),
            OpCode::Heartbeat => self.send_heartbeat().await,
            OpCode::Reconnect => {
                tracing::info!(shard = self.shared.shard.number(), "opcode 7 reconnect");
                Some(self.reconnect_close(true, false).await)
            }
            OpCode::InvalidSession => {
                let resumable = envelope.data::<bool>().unwrap_or(false);
                tracing::warn!(
                    shard = self.shared.shard.number(),
                    resumable,
                    "invalid session"
                );
                if resumable && self.shared.resume_info().is_some() {
                    self.try_resume = true;
                    self.send_resume().await
                } else {
                    self.try_resume = false;
                    self.shared.clear_resume();
                    let wait = Duration::from_millis(1_000 + fastrand::u64(0..4_000));
                    self.identify_at = Some(Instant::now() + wait);
                    None
                }
            }
            OpCode::Hello => {
                let hello = match envelope.data::<Hello>() {
                    Ok(h) => h,
                    Err(err) => {
                        tracing::debug!(%err, "bad hello");
                        return None;
                    }
                };
                let interval = Duration::from_millis(hello.heartbeat_interval);
                self.heartbeat_interval = Some(interval);
                let jitter_ms = fastrand::u128(0..=interval.as_millis());
                let jitter = Duration::from_millis(u64::try_from(jitter_ms).unwrap_or(u64::MAX));
                self.next_heartbeat = Some(Instant::now() + jitter);
                self.awaiting_ack = false;
                if self.try_resume && self.shared.resume_info().is_some() {
                    self.send_resume().await
                } else {
                    self.send_identify().await
                }
            }
            OpCode::HeartbeatAck => {
                self.awaiting_ack = false;
                let now = Instant::now();
                *lock(&self.shared.last_ack) = Some(now);
                if let Some(sent) = self.heartbeat_sent_at.take() {
                    lock(&self.shared.latency).push(now.saturating_duration_since(sent));
                }
                None
            }
            other => {
                tracing::debug!(
                    shard = self.shared.shard.number(),
                    op = ?other,
                    "unknown opcode"
                );
                None
            }
        }
    }

    fn on_dispatch(&mut self, envelope: &Envelope<'_>) -> Option<SessionEnd> {
        if let Some(s) = envelope.s {
            self.seq = Some(s);
            if let Some(info) = lock(&self.shared.resume).as_mut() {
                info.seq = s;
            }
        }
        let name = envelope.t.as_deref().unwrap_or("");
        if name == "READY" {
            match Event::from_dispatch(name, envelope.d) {
                Ok(Event::Ready(ready)) => {
                    self.shared.store_resume(ResumeInfo {
                        session_id: ready.session_id.clone(),
                        seq: self.seq.unwrap_or(0),
                        resume_gateway_url: ready.resume_gateway_url.clone(),
                    });
                    let guilds = u32::try_from(ready.guilds.len()).unwrap_or(u32::MAX);
                    self.shared.guild_count.store(guilds, Ordering::Relaxed);
                    self.shared.set_state(ShardState::Active);
                    self.phase_deadline = None;
                    self.active_since = Some(Instant::now());
                    self.deliver(Event::Ready(ready));
                }
                Ok(other) => self.deliver(other),
                Err(err) => {
                    tracing::warn!(%err, "ready deserialize failed");
                }
            }
            return None;
        }
        if name == "RESUMED" {
            self.shared.set_state(ShardState::Active);
            self.phase_deadline = None;
            self.active_since = Some(Instant::now());
            if self.shared.event_types.wants("RESUMED") {
                self.deliver(Event::Resumed);
            }
            return None;
        }
        if name == "GUILD_MEMBERS_CHUNK" {
            match Event::from_dispatch(name, envelope.d) {
                Ok(Event::GuildMembersChunk(chunk)) => {
                    self.on_member_chunk(&chunk);
                    if self.shared.event_types.wants(name) {
                        self.deliver(Event::GuildMembersChunk(chunk));
                    }
                }
                Ok(other) => self.deliver(other),
                Err(err) => tracing::warn!(%err, "chunk deserialize failed; event dropped"),
            }
            return None;
        }
        if !self.shared.event_types.wants(name) {
            return None;
        }
        match Event::from_dispatch(name, envelope.d) {
            Ok(event) => self.deliver(event),
            Err(err) => tracing::warn!(
                shard = self.shared.shard.number(),
                event = name,
                %err,
                "dispatch deserialize failed; event dropped"
            ),
        }
        None
    }

    fn on_member_chunk(&mut self, chunk: &GuildMembersChunk) {
        let Some(nonce) = chunk.nonce.as_deref() else {
            return;
        };
        let Some(mut pending) = self.members.remove(nonce) else {
            return;
        };
        pending.members.extend(chunk.members.iter().cloned());
        if chunk.chunk_index + 1 >= chunk.chunk_count {
            let _ = pending.done.send(Ok(pending.members));
        } else {
            self.members.insert(nonce.into(), pending);
        }
    }

    fn deliver(&mut self, event: Event) {
        match self
            .shared
            .events
            .push_event(event, self.shared.backpressure)
        {
            Push::Disconnect => {
                self.phase_deadline = Some(Instant::now());
                self.try_resume = true;
            }
            Push::Full(event) => self.pending_event = Some(event),
            Push::Ok => {}
        }
    }

    fn is_stable(&self) -> bool {
        self.active_since
            .is_some_and(|t| Instant::now() >= t + STABLE_ACTIVE)
    }

    fn reconnect_end(&self, use_resume: bool, apply_backoff: bool) -> SessionEnd {
        SessionEnd::Reconnect {
            use_resume,
            apply_backoff,
            stable: self.is_stable(),
        }
    }

    async fn on_heartbeat(&mut self) -> Option<SessionEnd> {
        if self.awaiting_ack {
            tracing::warn!(shard = self.shared.shard.number(), "zombie connection");
            return Some(self.reconnect_close(true, true).await);
        }
        self.send_heartbeat().await
    }

    async fn send_heartbeat(&mut self) -> Option<SessionEnd> {
        self.json_buf.clear();
        if let Err(err) = Heartbeat(self.seq).write_json(&mut self.json_buf) {
            tracing::debug!(%err, "heartbeat serialize");
            return None;
        }
        let now = Instant::now();
        if !self.outbound.can_send_heartbeat(now) {
            tracing::warn!("heartbeat starved by outbound limiter");
        }
        if let Some(end) = self.send_buf().await {
            return Some(end);
        }
        self.outbound.record(now);
        self.awaiting_ack = true;
        self.heartbeat_sent_at = Some(now);
        if let Some(interval) = self.heartbeat_interval {
            self.next_heartbeat = Some(now + interval);
        }
        None
    }

    async fn send_identify(&mut self) -> Option<SessionEnd> {
        self.shared
            .identify_queue
            .acquire(self.shared.shard.number())
            .await;
        self.shared.set_state(ShardState::Identifying);
        let properties = ConnectionProperties {
            os: std::env::consts::OS,
            browser: LIBRARY_NAME,
            device: LIBRARY_NAME,
        };
        let mut identify =
            Identify::new(self.shared.token.expose(), properties, self.shared.intents);
        identify.compress = false;
        identify.large_threshold = Some(self.shared.large_threshold);
        identify.shard = Some(self.shared.shard.identify_pair());
        identify.presence = self.shared.presence.as_ref();
        self.json_buf.clear();
        if let Err(err) = identify.write_json(&mut self.json_buf) {
            return Some(SessionEnd::Fatal(GatewayError::Serialize { source: err }));
        }
        if let Some(end) = self.send_user_now().await {
            return Some(end);
        }
        self.phase_deadline = Some(Instant::now() + READY_TIMEOUT);
        None
    }

    async fn send_resume(&mut self) -> Option<SessionEnd> {
        let Some(info) = self.shared.resume_info() else {
            return self.send_identify().await;
        };
        self.shared.set_state(ShardState::Resuming);
        let resume = Resume::new(self.shared.token.expose(), &info.session_id, info.seq);
        self.json_buf.clear();
        if let Err(err) = resume.write_json(&mut self.json_buf) {
            return Some(SessionEnd::Fatal(GatewayError::Serialize { source: err }));
        }
        if let Some(end) = self.send_user_now().await {
            return Some(end);
        }
        self.phase_deadline = Some(Instant::now() + READY_TIMEOUT);
        None
    }

    async fn flush_user(&mut self) -> Option<SessionEnd> {
        while !self.user_q.is_empty() && self.outbound.can_send_user(Instant::now()) {
            let cmd = self.user_q.pop_front()?;
            match cmd {
                UserCommand::Close { mode, done } => {
                    return Some(self.shutdown(mode, Some(done)).await);
                }
                UserCommand::Payload { bytes, done } => {
                    self.json_buf.clear();
                    self.json_buf.extend_from_slice(&bytes);
                    let result = self.send_user_now().await;
                    let send_ok = result.is_none();
                    let _ = done.send(if send_ok {
                        Ok(())
                    } else {
                        Err(GatewayError::Stopped)
                    });
                    if let Some(end) = result {
                        return Some(end);
                    }
                }
                UserCommand::RequestMembers { nonce, bytes, done } => {
                    self.json_buf.clear();
                    self.json_buf.extend_from_slice(&bytes);
                    let result = self.send_user_now().await;
                    if let Some(end) = result {
                        let _ = done.send(Err(GatewayError::Stopped));
                        return Some(end);
                    }
                    self.members.insert(
                        nonce,
                        PendingMembers {
                            members: Vec::new(),
                            done,
                            deadline: Instant::now() + self.shared.member_timeout,
                        },
                    );
                }
            }
        }
        None
    }

    fn expire_member_requests(&mut self) {
        let now = Instant::now();
        let timeout_after = self.shared.member_timeout;
        let expired: Vec<Box<str>> = self
            .members
            .iter()
            .filter(|(_, pending)| now >= pending.deadline)
            .map(|(nonce, _)| nonce.clone())
            .collect();
        for nonce in expired {
            if let Some(pending) = self.members.remove(&nonce) {
                let _ = pending.done.send(Err(GatewayError::Timeout(timeout_after)));
            }
        }
    }

    async fn send_user_now(&mut self) -> Option<SessionEnd> {
        let now = Instant::now();
        if let Some(end) = self.send_buf().await {
            return Some(end);
        }
        self.outbound.record(now);
        None
    }

    async fn send_buf(&mut self) -> Option<SessionEnd> {
        if self.json_buf.len() > MAX_PAYLOAD_BYTES {
            tracing::error!("outbound payload too large");
            return None;
        }
        let bytes = Bytes::copy_from_slice(&self.json_buf);
        match self.conn.send(Outgoing::Text(bytes)).await {
            Ok(()) => None,
            Err(err) => {
                tracing::warn!(%err, "send failed");
                Some(self.reconnect_end(self.shared.resume_info().is_some(), true))
            }
        }
    }

    async fn reconnect_close(&mut self, use_resume: bool, apply_backoff: bool) -> SessionEnd {
        self.reconnect_close_now(use_resume, apply_backoff).await
    }

    async fn reconnect_close_now(&mut self, use_resume: bool, apply_backoff: bool) -> SessionEnd {
        let _ = timeout(
            CLOSE_TIMEOUT,
            self.conn.send(Outgoing::Close {
                code: 4000,
                reason: "reconnect".into(),
            }),
        )
        .await;
        self.reconnect_end(use_resume, apply_backoff)
    }

    async fn shutdown(
        &mut self,
        mode: CloseMode,
        done: Option<oneshot::Sender<Option<ResumeInfo>>>,
    ) -> SessionEnd {
        self.shared.set_state(ShardState::ShuttingDown);
        let (code, resume) = match mode {
            CloseMode::Resumable => (4000, self.shared.resume_info()),
            CloseMode::Terminate => {
                self.shared.clear_resume();
                (1000, None)
            }
        };
        let _ = timeout(
            CLOSE_TIMEOUT,
            self.conn.send(Outgoing::Close {
                code,
                reason: "close".into(),
            }),
        )
        .await;
        if let Some(done) = done {
            let _ = done.send(resume.clone());
        }
        SessionEnd::Shutdown { resume }
    }
}

async fn sleep_until_opt(when: Option<Instant>) {
    match when {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}
