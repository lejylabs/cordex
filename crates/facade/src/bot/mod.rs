//! The simple layer: a whole bot in a few lines.
//!
//! [`Bot`] wires gateway, REST and interactions together. Register
//! handlers as plain async closures, call [`Bot::run`], done:
//!
//! ```no_run
//! use cordex::prelude::*;
//!
//! # async fn run() -> cordex::Result<()> {
//! Bot::new("token")
//!     .command(Command::slash("ping", "Replies with pong"), |ctx: CommandContext| async move {
//!         ctx.reply("Pong!").await
//!     })
//!     .on_message(|ctx: MessageContext| async move {
//!         if ctx.content() == "!hi" {
//!             ctx.reply("Hi!").await?;
//!         }
//!         Ok(())
//!     })
//!     .run()
//!     .await
//! # }
//! ```
//!
//! What it does for you:
//!
//! - **Intents** follow from the handlers: none for a pure command bot
//!   (interactions need no intent), message intents with
//!   [`on_message`](Bot::on_message), `GUILDS` with [`cache`](Bot::cache).
//!   Privileged ones only when asked for
//!   ([`message_content`](Bot::message_content), [`intents`](Bot::intents)).
//! - **Only needed events are parsed:** the gateway event filter is set
//!   from the handlers and the cache config, everything else is skipped
//!   after reading its name.
//! - **Shards** are automatic (Discord's recommendation); reconnects and
//!   resumes happen inside the gateway.
//! - **Commands** are registered with one bulk overwrite on the first
//!   `READY` (global, or per guild with [`guild_commands`](Bot::guild_commands)),
//!   skipped when Discord already has the same definitions
//!   ([`register_commands(false)`](Bot::register_commands) turns it off).
//! - **Interactions** are deferred automatically after 2.5 s if a
//!   handler has not answered yet; `ctx.reply` then edits the deferred
//!   response.
//! - **Errors and panics** in handlers are logged (`tracing`) and never
//!   stop the bot. An unanswered interaction gets an ephemeral error
//!   message (see [`error_message`](Bot::error_message)).
//! - **No cache** unless [`cache`](Bot::cache) is called.
//!
//! Everything below stays reachable: `ctx.http()`, `ctx.shards()`,
//! `ctx.handle()`, [`on_event`](Bot::on_event) for any event.

mod context;
mod cooldown;
mod sync;

pub use cooldown::Cooldowns;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;

use cordex_gateway::{CloseMode, EventTypeFlags, ManagerEvent, ShardId, ShardManager, Standby};
use cordex_http::{Client, Route};
use cordex_interactions::handle::{DEFAULT_AUTO_DEFER, ResponseState};
use cordex_interactions::{Command, InteractionHandle, Reply};
use cordex_model::Message;
use cordex_model::gateway::Intents;
use cordex_model::gateway::event::{Event, Ready};
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, GuildMarker, UserMarker};
use cordex_model::interaction::{Interaction, InteractionData, InteractionType};
use cordex_model::message::AllowedMentions;

pub use context::{
    AutocompleteContext, ButtonClicks, CommandContext, ComponentContext, Context, EventContext,
    MessageContext, ModalContext, ReadyContext,
};

use crate::{Error, Result};

type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type Handler<C> = Arc<dyn Fn(C) -> BoxFuture + Send + Sync>;
type ErrorHandler =
    Arc<dyn Fn(Context, Error) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
type BeforeCommand =
    Arc<dyn Fn(CommandContext) -> Pin<Box<dyn Future<Output = Result<bool>> + Send>> + Send + Sync>;

fn boxed<C, F, Fut>(f: F) -> Handler<C>
where
    F: Fn(C) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |ctx| Box::pin(f(ctx)))
}

/// Default interval of the reshard check ([`Bot::reshard_interval`]).
pub const DEFAULT_RESHARD_INTERVAL: Duration = Duration::from_hours(8);

/// Default ephemeral answer when a handler fails before answering.
pub const DEFAULT_ERROR_MESSAGE: &str = "Something went wrong while handling this.";

#[derive(Default)]
struct Handlers {
    commands: HashMap<Box<str>, Handler<CommandContext>>,
    autocomplete: HashMap<Box<str>, Handler<AutocompleteContext>>,
    components: HashMap<Box<str>, Handler<ComponentContext>>,
    modals: HashMap<Box<str>, Handler<ModalContext>>,
    message: Vec<Handler<MessageContext>>,
    ready: Vec<Handler<ReadyContext>>,
    event: Vec<Handler<EventContext>>,
    before_command: Option<BeforeCommand>,
    on_error: Option<ErrorHandler>,
}

/// State every context points to.
struct Shared {
    http: Client,
    /// Current shard manager; replaced when the bot reshards.
    manager: std::sync::Mutex<Arc<ShardManager>>,
    #[cfg(feature = "cache")]
    cache: Option<Arc<cordex_cache::InMemoryCache>>,
    me: OnceLock<Id<UserMarker>>,
    handlers: Handlers,
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    error_message: Option<Box<str>>,
    auto_defer: bool,
    standby: Standby,
}

impl Shared {
    fn manager(&self) -> Arc<ShardManager> {
        Arc::clone(
            &self
                .manager
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// Builder and runner of a bot; see the [module docs](self).
#[must_use = "a Bot does nothing until `.run().await`"]
pub struct Bot {
    token: String,
    intents: Intents,
    shards: Option<u32>,
    commands: Vec<Command>,
    guild_commands: Option<Id<GuildMarker>>,
    register_commands: bool,
    handlers: Handlers,
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    error_message: Option<Box<str>>,
    auto_defer: bool,
    default_allowed_mentions: Option<AllowedMentions>,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
    extra_events: EventTypeFlags,
    reshard_interval: Option<Duration>,
    #[cfg(feature = "cache")]
    cache: Option<cordex_cache::CacheConfig>,
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot")
            .field("intents", &self.intents)
            .field("shards", &self.shards)
            .field("commands", &self.commands.len())
            .finish_non_exhaustive()
    }
}

impl Bot {
    /// New bot with the token from the developer portal.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            intents: Intents::empty(),
            shards: None,
            commands: Vec::new(),
            guild_commands: None,
            register_commands: true,
            handlers: Handlers::default(),
            data: HashMap::new(),
            error_message: Some(DEFAULT_ERROR_MESSAGE.into()),
            auto_defer: true,
            default_allowed_mentions: None,
            presence: None,
            extra_events: EventTypeFlags::empty(),
            reshard_interval: Some(DEFAULT_RESHARD_INTERVAL),
            #[cfg(feature = "cache")]
            cache: None,
        }
    }

    /// Mentions applied to every message that does not set its own.
    pub fn default_allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
        self.default_allowed_mentions = Some(mentions);
        self
    }

    /// Initial presence sent with `IDENTIFY` on every shard (also preserved
    /// across automatic reshards).
    pub fn presence(mut self, presence: cordex_model::gateway::presence::UpdatePresence) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Extra gateway events to parse so [`Context::wait_for`] can see them.
    pub fn parse_events(mut self, events: EventTypeFlags) -> Self {
        self.extra_events |= events;
        self
    }

    /// Add gateway intents on top of the derived ones (for
    /// [`on_event`](Self::on_event) handlers or the cache).
    pub fn intents(mut self, intents: Intents) -> Self {
        self.intents |= intents;
        self
    }

    /// Turn on the privileged `MESSAGE_CONTENT` intent (also enable it in
    /// the developer portal), so [`MessageContext::content`] is filled.
    pub fn message_content(self) -> Self {
        self.intents(Intents::MESSAGE_CONTENT)
    }

    /// How often to check whether Discord recommends more shards and, if
    /// so, reshard while running (default every 8 hours; `None` turns it
    /// off). Only with automatic sharding: a fixed [`shards`](Self::shards)
    /// count is kept.
    ///
    /// Resharding starts a second set of shards with the new count, waits
    /// until all of them are `READY`, switches over and closes the old ones,
    /// so the bot stays online. It needs enough identifies left in the
    /// session start limit, otherwise it waits for the next check.
    pub fn reshard_interval(mut self, interval: Option<Duration>) -> Self {
        self.reshard_interval = interval;
        self
    }

    /// Fixed shard count instead of Discord's recommendation (turns off
    /// resharding).
    pub fn shards(mut self, count: u32) -> Self {
        self.shards = Some(count.max(1));
        self
    }

    /// Register commands in this guild only. They update instantly, so
    /// this is handy while developing; global commands are the default.
    pub fn guild_commands(mut self, guild: Id<GuildMarker>) -> Self {
        self.guild_commands = Some(guild);
        self
    }

    /// Switch the automatic command registration off (default on). The
    /// handlers still run; use this when commands are registered
    /// elsewhere, e.g. by a deploy step or another process of the same
    /// application.
    pub fn register_commands(mut self, enabled: bool) -> Self {
        self.register_commands = enabled;
        self
    }

    /// Ephemeral message sent when a handler fails or panics before
    /// answering its interaction; `None` sends nothing.
    pub fn error_message(mut self, message: Option<&str>) -> Self {
        self.error_message = message.map(Into::into);
        self
    }

    /// Switch automatic deferring of commands and modals off (default
    /// on: after 2.5 s without an answer).
    pub fn auto_defer(mut self, enabled: bool) -> Self {
        self.auto_defer = enabled;
        self
    }

    /// Cache with this config, fed from every event. Reach it with
    /// `ctx.cache()`. Off unless called.
    #[cfg(feature = "cache")]
    ///
    /// Adds the `GUILDS` intent (guild data comes with `GUILD_CREATE`);
    /// privileged intents for members or presences stay your choice
    /// ([`intents`](Self::intents)).
    pub fn cache(mut self, config: cordex_cache::CacheConfig) -> Self {
        if !config.wanted_events().is_empty() {
            self.intents |= Intents::GUILDS;
        }
        self.cache = Some(config);
        self
    }

    /// Attach shared state accessible in every handler via
    /// [`Context::data`], e.g. a database pool or configuration struct.
    /// Calling `data` again with the same type replaces the previous value.
    pub fn data<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        self.data.insert(TypeId::of::<T>(), Box::new(value));
        self
    }

    /// Global hook invoked whenever a handler returns `Err`. Runs in
    /// addition to tracing logs (and the default ephemeral interaction error
    /// message, unless disabled with [`error_message(None)`](Self::error_message)).
    pub fn on_error<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Error) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.handlers.on_error = Some(Arc::new(move |ctx, err| Box::pin(handler(ctx, err))));
        self
    }

    /// Hook run before every slash, user or message command handler.
    /// Return `Ok(true)` to proceed with the command, or `Ok(false)` to stop
    /// (for cooldowns, maintenance mode, blacklists, or logging).
    pub fn before_command<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<bool>> + Send + 'static,
    {
        self.handlers.before_command = Some(Arc::new(move |ctx| Box::pin(handler(ctx))));
        self
    }

    /// Register a command and its handler. Slash, user and message
    /// commands are routed by name.
    pub fn command<F, Fut>(mut self, command: Command, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .commands
            .insert(command.name().into(), boxed(handler));
        self.commands.retain(|c| c.name() != command.name());
        self.commands.push(command);
        self
    }

    /// Shortcut for registering a slash command without options.
    pub fn slash<F, Fut>(
        self,
        name: impl AsRef<str>,
        description: impl AsRef<str>,
        handler: F,
    ) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.command(Command::slash(name, description), handler)
    }

    /// Shortcut for registering a user context-menu command.
    pub fn user_command<F, Fut>(self, name: impl AsRef<str>, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.command(Command::user(name), handler)
    }

    /// Shortcut for registering a message context-menu command.
    pub fn message_command<F, Fut>(self, name: impl AsRef<str>, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.command(Command::message(name), handler)
    }

    /// Autocomplete handler for the options of command `name` (mark the
    /// options with `.autocomplete()`).
    pub fn autocomplete<F, Fut>(mut self, command: &str, handler: F) -> Self
    where
        F: Fn(AutocompleteContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .autocomplete
            .insert(command.into(), boxed(handler));
        self
    }

    /// Handler for buttons and select menus with this `custom_id`. Ids
    /// with an argument route by the part before the first `:`
    /// (`"vote:42"` goes to `"vote"`, see
    /// [`ComponentContext::custom_id_arg`]).
    pub fn on_component<F, Fut>(mut self, custom_id: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .components
            .insert(custom_id.into(), boxed(handler));
        self
    }

    /// Alias of [`on_component`](Self::on_component) for buttons.
    pub fn on_button<F, Fut>(self, custom_id: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.on_component(custom_id, handler)
    }

    /// Handler for submitted modals with this `custom_id` (same `:` rule
    /// as [`on_component`](Self::on_component)).
    pub fn on_modal<F, Fut>(mut self, custom_id: &str, handler: F) -> Self
    where
        F: Fn(ModalContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .modals
            .insert(custom_id.into(), boxed(handler));
        self
    }

    /// Handler for new messages (guilds and DMs). Adds the
    /// `GUILD_MESSAGES` and `DIRECT_MESSAGES` intents; for the text in
    /// guilds also call [`message_content`](Self::message_content).
    pub fn on_message<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(MessageContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES;
        self.handlers.message.push(boxed(handler));
        self
    }

    /// Handler called on every `READY` (once per shard, again after a
    /// full reconnect).
    pub fn on_ready<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ReadyContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.ready.push(boxed(handler));
        self
    }

    /// Handler for every gateway event; add the intents it needs with
    /// [`intents`](Self::intents).
    pub fn on_event<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(EventContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.event.push(boxed(handler));
        self
    }

    /// Events worth parsing: only what a handler or the cache reads. The
    /// gateway skips the rest after reading the event name (READY always
    /// arrives).
    fn event_types(&self) -> EventTypeFlags {
        let h = &self.handlers;
        if !h.event.is_empty() {
            return EventTypeFlags::all();
        }
        let mut names: Vec<&str> = Vec::new();
        if !(h.commands.is_empty()
            && h.autocomplete.is_empty()
            && h.components.is_empty()
            && h.modals.is_empty())
        {
            names.push("INTERACTION_CREATE");
        }
        if !h.message.is_empty() {
            names.push("MESSAGE_CREATE");
        }
        #[cfg(feature = "cache")]
        if let Some(config) = &self.cache {
            names.extend(config.wanted_events());
        }
        names.into_iter().collect::<EventTypeFlags>() | self.extra_events
    }

    /// Check the commands, connect and handle events until a shard fails
    /// fatally (bad token, disallowed intents, …); that error is returned.
    pub async fn run(self) -> Result<()> {
        self.run_until(std::future::pending()).await
    }

    /// Like [`run`](Self::run), but shuts down all shards cleanly when
    /// `Ctrl+C` (or `SIGTERM` on Unix) is received.
    pub async fn run_until_ctrl_c(self) -> Result<()> {
        self.run_until(shutdown_signal()).await
    }

    /// Like [`run`](Self::run), but shuts down all shards cleanly when
    /// `shutdown` completes.
    pub async fn run_until(self, shutdown: impl Future<Output = ()> + Send) -> Result<()> {
        for command in &self.commands {
            command.validate()?;
        }
        if !self.handlers.message.is_empty() && !self.intents.contains(Intents::MESSAGE_CONTENT) {
            tracing::info!(
                "on_message without MESSAGE_CONTENT: guild message text is empty unless the bot \
                 is mentioned; call Bot::message_content() to turn it on"
            );
        }

        let mut http = Client::builder(&self.token);
        if let Some(mentions) = self.default_allowed_mentions.clone() {
            http = http.default_allowed_mentions(mentions);
        }
        let http = http.build()?;
        let event_types = self.event_types();
        let fixed_shards = self.shards;
        let presence = self.presence;
        let mut manager = build_manager(
            &self.token,
            self.intents,
            &http,
            event_types,
            fixed_shards,
            presence.clone(),
        )
        .await?;
        let mut events = manager.events();

        let shared = Arc::new(Shared {
            http,
            manager: std::sync::Mutex::new(Arc::new(manager)),
            #[cfg(feature = "cache")]
            cache: self
                .cache
                .map(|config| Arc::new(cordex_cache::InMemoryCache::new(config))),
            me: OnceLock::new(),
            handlers: self.handlers,
            data: self.data,
            error_message: self.error_message,
            auto_defer: self.auto_defer,
            standby: Standby::new(),
        });
        let commands: Arc<[Command]> = self.commands.into();
        let guild_commands = self.guild_commands;
        let registered = AtomicBool::new(!self.register_commands);

        shared.manager().start().await?;

        // Resharding: a task prepares a new manager and hands it over here.
        let (switch_tx, mut switch_rx) = tokio::sync::mpsc::channel::<Reshard>(1);
        if fixed_shards.is_none()
            && let Some(interval) = self.reshard_interval
        {
            tokio::spawn(reshard_loop(
                Arc::clone(&shared),
                self.token.clone(),
                self.intents,
                event_types,
                presence,
                interval,
                switch_tx,
            ));
        }

        let mut shutdown = std::pin::pin!(shutdown);
        loop {
            let item = tokio::select! {
                () = &mut shutdown => {
                    tracing::info!("shutdown signal received; closing shards");
                    shared.manager().shutdown(CloseMode::Terminate).await;
                    return Ok(());
                }
                item = events.recv() => item,
                Some(next) = switch_rx.recv() => {
                    let old = std::mem::replace(
                        &mut *shared.manager.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                        Arc::new(next.manager),
                    );
                    events = next.events;
                    tracing::info!(
                        shards = shared.manager().total_shards(),
                        "resharded; closing the old shards"
                    );
                    tokio::spawn(async move {
                        old.shutdown(CloseMode::Terminate).await;
                    });
                    continue;
                }
            };
            let Some(item) = item else { break };
            on_manager_event(&shared, item, &registered, guild_commands, &commands).await?;
        }
        Ok(())
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async {
                if let Some(s) = term.as_mut() {
                    let _ = s.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Handle one item of the shard manager stream; a fatal shard error stops
/// the bot.
async fn on_manager_event(
    shared: &Arc<Shared>,
    item: ManagerEvent,
    registered: &AtomicBool,
    guild_commands: Option<Id<GuildMarker>>,
    commands: &Arc<[Command]>,
) -> Result<()> {
    match item {
        ManagerEvent::Dispatch { shard, event } => {
            if !registered.load(Ordering::Relaxed)
                && let Event::Ready(ready) = &event
                && !registered.swap(true, Ordering::Relaxed)
            {
                tokio::spawn(register_commands(
                    shared.http.clone(),
                    ready.application.id,
                    guild_commands,
                    Arc::clone(commands),
                ));
            }
            dispatch(shared, shard, event);
        }
        ManagerEvent::Error { shard, error } => {
            tracing::error!(shard = shard.number(), %error, "shard failed, stopping the bot");
            shared.manager().shutdown(CloseMode::Terminate).await;
            return Err(Error::Gateway(error));
        }
        ManagerEvent::RecommendedShardCount {
            running,
            recommended,
        } => {
            tracing::info!(
                running,
                recommended,
                "Discord recommends more shards; the reshard check picks it up"
            );
        }
        _ => {}
    }
    Ok(())
}

async fn build_manager(
    token: &str,
    intents: Intents,
    http: &Client,
    event_types: EventTypeFlags,
    fixed_shards: Option<u32>,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
) -> Result<ShardManager> {
    let mut builder = ShardManager::builder(token, intents)
        .http(http.clone())
        .event_types(event_types);
    if let Some(presence) = presence {
        builder = builder.presence(presence);
    }
    let builder = match fixed_shards {
        Some(n) => builder.shards(0..n, n),
        None => builder.automatic(),
    };
    Ok(builder.build().await?)
}

/// A started manager whose shards are all `READY`, ready to take over.
struct Reshard {
    manager: ShardManager,
    events: cordex_gateway::ManagerEvents,
}

/// Every `interval`: if Discord recommends more shards than running and
/// enough identifies are left, start a new manager, wait for all its
/// shards to be `READY` (dropping their events, the old shards still
/// deliver), then hand it to the run loop.
async fn reshard_loop(
    shared: Arc<Shared>,
    token: String,
    intents: Intents,
    event_types: EventTypeFlags,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
    interval: Duration,
    switch: tokio::sync::mpsc::Sender<Reshard>,
) {
    loop {
        tokio::time::sleep(interval).await;
        let running = shared.manager().total_shards();
        let info = match shared.http.gateway_bot().await {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(%error, "reshard check: GET /gateway/bot failed");
                continue;
            }
        };
        if !needs_reshard(running, info.shards, info.session_start_limit.remaining) {
            continue;
        }
        tracing::info!(running, recommended = info.shards, "resharding");
        match prepare_reshard(&shared.http, &token, intents, event_types, presence.clone()).await {
            Ok(next) => {
                if switch.send(next).await.is_err() {
                    return; // the bot stopped
                }
            }
            Err(error) => tracing::warn!(%error, "resharding failed; keeping the current shards"),
        }
    }
}

/// More shards recommended and enough identifies left for all of them.
fn needs_reshard(running: u32, recommended: u32, identifies_left: u32) -> bool {
    if recommended <= running {
        return false;
    }
    if identifies_left < recommended {
        tracing::warn!(
            recommended,
            identifies_left,
            "Discord recommends more shards, but too few identifies are left; retrying later"
        );
        return false;
    }
    true
}

async fn prepare_reshard(
    http: &Client,
    token: &str,
    intents: Intents,
    event_types: EventTypeFlags,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
) -> Result<Reshard> {
    let mut manager = build_manager(token, intents, http, event_types, None, presence).await?;
    let mut events = manager.events();
    manager.start().await?;
    let total = manager.total_shards();
    // Identify pacing: ~5 s per bucket; allow generous time for large bots.
    let deadline = Duration::from_secs(60 + u64::from(total) * 6);
    let mut ready = std::collections::HashSet::new();
    let waited = tokio::time::timeout(deadline, async {
        while let Some(item) = events.recv().await {
            match item {
                ManagerEvent::Dispatch {
                    shard,
                    event: Event::Ready(_),
                } => {
                    ready.insert(shard.number());
                    if ready.len() == total as usize {
                        return Ok(());
                    }
                }
                ManagerEvent::Error { error, .. } => return Err(error),
                _ => {}
            }
        }
        Ok(())
    })
    .await;
    match waited {
        Ok(Ok(())) if ready.len() == total as usize => Ok(Reshard { manager, events }),
        Ok(Err(error)) => {
            manager.shutdown(CloseMode::Terminate).await;
            Err(Error::Gateway(error))
        }
        _ => {
            manager.shutdown(CloseMode::Terminate).await;
            Err(Error::Gateway(cordex_gateway::GatewayError::Validation(
                "new shards did not become ready in time".into(),
            )))
        }
    }
}

/// One bulk overwrite with all commands, skipped when Discord already
/// has exactly these (only when there are any, so a bot without commands
/// never wipes commands set elsewhere).
async fn register_commands(
    http: Client,
    app: Id<ApplicationMarker>,
    guild: Option<Id<GuildMarker>>,
    commands: Arc<[Command]>,
) {
    if commands.is_empty() {
        return;
    }
    let route = match guild {
        Some(guild) => Route::raw(
            "/applications/{application_id}/guilds/{guild_id}/commands",
            (app, guild),
        ),
        None => Route::raw("/applications/{application_id}/commands", app),
    };
    match http.get(route).send_json::<Vec<serde_json::Value>>().await {
        Ok(remote) => {
            let local: Vec<serde_json::Value> = commands
                .iter()
                .filter_map(|c| serde_json::to_value(c).ok())
                .collect();
            if local.len() == commands.len() && sync::commands_match(&local, &remote) {
                tracing::info!(count = commands.len(), ?guild, "commands unchanged");
                return;
            }
        }
        Err(error) => tracing::debug!(%error, "fetching registered commands failed"),
    }
    let list = commands.iter().cloned();
    let result = match guild {
        Some(guild) => http.bulk_overwrite_guild_commands(app, guild, list).await,
        None => http.bulk_overwrite_global_commands(app, list).await,
    };
    match result {
        Ok(list) => tracing::info!(count = list.len(), ?guild, "registered commands"),
        Err(error) => tracing::error!(%error, ?guild, "registering commands failed"),
    }
}

fn dispatch(shared: &Arc<Shared>, shard_id: ShardId, event: Event) {
    let ctx = Context {
        shared: Arc::clone(shared),
        shard: shard_id,
    };
    #[cfg(feature = "cache")]
    if let Some(cache) = &shared.cache {
        cache.update(&event);
    }
    let taken = shared.standby.process(&event) > 0;
    // A component interaction a waiter took (`ctx.wait_for_button`) is
    // answered there; routing it to `on_component` too would answer twice.
    if taken
        && let Event::InteractionCreate(interaction) = &event
        && interaction.kind == InteractionType::MessageComponent
    {
        return;
    }
    let handlers = &shared.handlers;
    if handlers.event.is_empty() {
        route_owned(ctx, event);
        return;
    }
    // `on_event` handlers share one `Arc<Event>`; the typed handlers get
    // copies of only the parts they read.
    let event = Arc::new(event);
    for handler in &handlers.event {
        spawn_guarded(
            "event",
            ctx.clone(),
            handler(EventContext {
                ctx: ctx.clone(),
                event: Arc::clone(&event),
            }),
        );
    }
    match &*event {
        Event::Ready(ready) => route_ready(&ctx, || Arc::new(Ready::clone(ready)), ready.user.id),
        Event::MessageCreate(message) => route_message(&ctx, || Arc::new(Message::clone(message))),
        Event::InteractionCreate(interaction) => route_interaction(ctx, interaction.clone()),
        _ => {}
    }
}

fn route_owned(ctx: Context, event: Event) {
    match event {
        Event::Ready(ready) => {
            let id = ready.user.id;
            route_ready(&ctx, || Arc::from(ready), id);
        }
        Event::MessageCreate(message) => route_message(&ctx, || Arc::from(message)),
        Event::InteractionCreate(interaction) => route_interaction(ctx, interaction),
        _ => {}
    }
}

/// `ready` is only called when a handler needs the payload.
fn route_ready(ctx: &Context, ready: impl FnOnce() -> Arc<Ready>, me: Id<UserMarker>) {
    let _ = ctx.shared.me.set(me);
    let handlers = &ctx.shared.handlers.ready;
    if handlers.is_empty() {
        return;
    }
    let ready = ready();
    for handler in handlers {
        spawn_guarded(
            "ready",
            ctx.clone(),
            handler(ReadyContext {
                ctx: ctx.clone(),
                ready: Arc::clone(&ready),
            }),
        );
    }
}

/// `message` is only called when a handler wants it (not for the bot's own
/// messages).
fn route_message(ctx: &Context, message: impl FnOnce() -> Arc<Message>) {
    let handlers = &ctx.shared.handlers.message;
    if handlers.is_empty() {
        return;
    }
    let message = message();
    if ctx.shared.me.get() == Some(&message.author.id) {
        return;
    }
    for handler in handlers {
        spawn_guarded(
            "message",
            ctx.clone(),
            handler(MessageContext {
                ctx: ctx.clone(),
                message: Arc::clone(&message),
            }),
        );
    }
}

/// Look up a component/modal handler: exact id first, then the part
/// before the first `:`.
fn lookup<'a, C>(map: &'a HashMap<Box<str>, Handler<C>>, id: &str) -> Option<&'a Handler<C>> {
    map.get(id)
        .or_else(|| id.split_once(':').and_then(|(prefix, _)| map.get(prefix)))
}

fn route_interaction(ctx: Context, mut interaction: Box<Interaction>) {
    let handle = InteractionHandle::new(ctx.shared.http.clone(), &interaction);
    let handlers = &ctx.shared.handlers;
    let auto_defer = ctx.shared.auto_defer;
    let error_message = ctx.shared.error_message.clone();
    let kind = interaction.kind;
    let error_ctx = ctx.clone();
    let future: BoxFuture = match interaction.data.take() {
        Some(InteractionData::Command(data))
            if kind == InteractionType::ApplicationCommandAutocomplete =>
        {
            let Some(handler) = handlers.autocomplete.get(&*data.name).cloned() else {
                tracing::debug!(command = %data.name, "no autocomplete handler");
                return;
            };
            handler(AutocompleteContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
            })
        }
        Some(InteractionData::Command(data)) => {
            let Some(handler) = handlers.commands.get(&*data.name).cloned() else {
                tracing::warn!(command = %data.name, "no handler for command");
                tokio::spawn(async move {
                    let reply = Reply::new("This command is not available.").ephemeral();
                    let _ = handle.respond(reply).await;
                });
                return;
            };
            if auto_defer {
                handle.auto_defer(DEFAULT_AUTO_DEFER, false);
            }
            let before = handlers.before_command.clone();
            let cmd_ctx = CommandContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
            };
            if let Some(before) = before {
                Box::pin(async move {
                    if before(cmd_ctx.clone()).await? {
                        handler(cmd_ctx).await
                    } else {
                        Ok(())
                    }
                })
            } else {
                handler(cmd_ctx)
            }
        }
        Some(InteractionData::MessageComponent(data)) => {
            let Some(handler) = lookup(&handlers.components, &data.custom_id).cloned() else {
                tracing::debug!(custom_id = %data.custom_id, "no component handler");
                return;
            };
            handler(ComponentContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
            })
        }
        Some(InteractionData::ModalSubmit(data)) => {
            let Some(handler) = lookup(&handlers.modals, &data.custom_id).cloned() else {
                tracing::debug!(custom_id = %data.custom_id, "no modal handler");
                return;
            };
            if auto_defer {
                handle.auto_defer(DEFAULT_AUTO_DEFER, false);
            }
            handler(ModalContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
            })
        }
        _ => return,
    };
    tokio::spawn(async move {
        if !guarded("interaction", &error_ctx, future).await
            && let Some(text) = error_message
        {
            answer_error(&handle, &text).await;
        }
    });
}

/// Spawn a handler; errors and panics are logged.
fn spawn_guarded(what: &'static str, ctx: Context, future: BoxFuture) {
    tokio::spawn(async move {
        let _ = guarded(what, &ctx, future).await;
    });
}

/// Await a handler and log a failure; `true` when it returned `Ok`.
/// Panics are caught while polling, so no extra task is needed.
async fn guarded(what: &'static str, ctx: &Context, future: BoxFuture) -> bool {
    match (CatchUnwind { future }).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "{what} handler failed");
            if let Some(on_error) = &ctx.shared.handlers.on_error {
                on_error(ctx.clone(), error).await;
            }
            false
        }
        Err(_) => {
            tracing::error!("{what} handler panicked");
            false
        }
    }
}

/// Polls a handler future and turns a panic into `Err`.
struct CatchUnwind {
    future: BoxFuture,
}

impl Future for CatchUnwind {
    type Output = std::thread::Result<Result<()>>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let future = &mut self.get_mut().future;
        // A panicked future is never polled again: the result is returned
        // right away and the future dropped with this struct.
        match std::panic::catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(result)) => Poll::Ready(Ok(result)),
            Err(panic) => Poll::Ready(Err(panic)),
        }
    }
}

/// Answer with `text` (ephemeral) if the interaction has no answer yet.
async fn answer_error(handle: &InteractionHandle, text: &str) {
    let result = match handle.state().await {
        ResponseState::Pending => handle.respond(Reply::new(text).ephemeral()).await,
        ResponseState::Deferred => handle.edit(Reply::new(text)).await.map(|_| ()),
        _ => Ok(()),
    };
    if let Err(error) = result {
        tracing::debug!(%error, "sending the error message failed");
    }
}

#[cfg(test)]
mod tests;
