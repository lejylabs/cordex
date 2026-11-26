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
mod health;
mod middleware;
mod module;
mod scheduler;
mod sync;

pub use cooldown::{CooldownScope, Cooldowns};
#[cfg(feature = "cache")]
pub use health::CacheHealth;
pub use health::{HealthSnapshot, ShardHealth};
pub use middleware::Next;
pub use module::BotModule;
pub use scheduler::{SchedulerHandle, ShutdownToken};

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
type CooldownMessage = Arc<dyn Fn(Duration) -> String + Send + Sync>;
type RouteParams = Box<[(Box<str>, Box<str>)]>;

fn boxed<C, F, Fut>(f: F) -> Handler<C>
where
    F: Fn(C) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |ctx| Box::pin(f(ctx)))
}

fn wrap_with_guards(
    handler: Handler<CommandContext>,
    guards: Arc<[BeforeCommand]>,
) -> Handler<CommandContext> {
    Arc::new(move |ctx: CommandContext| {
        let handler = Arc::clone(&handler);
        let guards = Arc::clone(&guards);
        Box::pin(async move {
            for guard in guards.iter() {
                if !guard(ctx.clone()).await? {
                    return Ok(());
                }
            }
            handler(ctx).await
        })
    })
}

struct PrefixRoute<C> {
    prefix: Box<str>,
    handler: Handler<C>,
}

enum PatternSegment {
    Literal(Box<str>),
    Param(Box<str>),
}

struct PatternRoute<C> {
    delimiter: char,
    segments: Box<[PatternSegment]>,
    literal_count: usize,
    handler: Handler<C>,
}

impl<C> PatternRoute<C> {
    fn parse(pattern: &str, handler: Handler<C>) -> Self {
        let delimiter = if pattern.contains(':') || !pattern.contains('/') {
            ':'
        } else {
            '/'
        };
        let mut literal_count = 0usize;
        let segments: Vec<PatternSegment> = pattern
            .split(delimiter)
            .map(|seg| {
                if let Some(name) = seg
                    .strip_prefix('{')
                    .and_then(|rest| rest.strip_suffix('}'))
                    && !name.is_empty()
                {
                    PatternSegment::Param(name.into())
                } else {
                    literal_count += 1;
                    PatternSegment::Literal(seg.into())
                }
            })
            .collect();
        Self {
            delimiter,
            segments: segments.into_boxed_slice(),
            literal_count,
            handler,
        }
    }

    fn matches(&self, id: &str) -> Option<RouteParams> {
        let parts: Vec<&str> = id.split(self.delimiter).collect();
        if parts.len() != self.segments.len() {
            return None;
        }
        let mut params = Vec::new();
        for (segment, actual) in self.segments.iter().zip(parts) {
            match segment {
                PatternSegment::Literal(expected) => {
                    if actual != &**expected {
                        return None;
                    }
                }
                PatternSegment::Param(name) => {
                    if actual.is_empty() {
                        return None;
                    }
                    params.push((name.clone(), Box::<str>::from(actual)));
                }
            }
        }
        Some(params.into_boxed_slice())
    }
}

/// Default interval of the reshard check ([`Bot::reshard_interval`]).
pub const DEFAULT_RESHARD_INTERVAL: Duration = Duration::from_hours(8);

/// Default ephemeral answer when a handler fails before answering.
pub const DEFAULT_ERROR_MESSAGE: &str = "Something went wrong while handling this.";

#[derive(Default)]
struct Handlers {
    commands: HashMap<Box<str>, Handler<CommandContext>>,
    command_cooldowns: HashMap<Box<str>, cooldown::CommandCooldown>,
    autocomplete: HashMap<Box<str>, Handler<AutocompleteContext>>,
    components: HashMap<Box<str>, Handler<ComponentContext>>,
    component_prefixes: Vec<PrefixRoute<ComponentContext>>,
    component_patterns: Vec<PatternRoute<ComponentContext>>,
    modals: HashMap<Box<str>, Handler<ModalContext>>,
    modal_prefixes: Vec<PrefixRoute<ModalContext>>,
    modal_patterns: Vec<PatternRoute<ModalContext>>,
    unknown_component: Option<Handler<ComponentContext>>,
    unknown_modal: Option<Handler<ModalContext>>,
    message: Vec<Handler<MessageContext>>,
    ready: Vec<Handler<ReadyContext>>,
    event: Vec<Handler<EventContext>>,
    guild_create: Vec<Handler<(Context, Arc<cordex_model::gateway::event::GuildCreate>)>>,
    member_join: Vec<Handler<(Context, Arc<cordex_model::Member>)>>,
    member_leave: Vec<
        Handler<(
            Context,
            Arc<cordex_model::gateway::event::GuildMemberRemove>,
        )>,
    >,
    reaction_add: Vec<
        Handler<(
            Context,
            Arc<cordex_model::gateway::event::MessageReactionAdd>,
        )>,
    >,
    voice_state: Vec<Handler<(Context, Arc<cordex_model::voice::VoiceState>)>>,
    before_command: Vec<BeforeCommand>,
    around_command: Vec<middleware::AroundCommand>,
    scheduled: Vec<scheduler::ScheduledTask>,
    on_error: Option<ErrorHandler>,
}

impl Handlers {
    fn merge(&mut self, other: Self) {
        self.commands.extend(other.commands);
        self.command_cooldowns.extend(other.command_cooldowns);
        self.autocomplete.extend(other.autocomplete);
        self.components.extend(other.components);
        self.component_prefixes.extend(other.component_prefixes);
        self.component_patterns.extend(other.component_patterns);
        self.modals.extend(other.modals);
        self.modal_prefixes.extend(other.modal_prefixes);
        self.modal_patterns.extend(other.modal_patterns);
        if other.unknown_component.is_some() {
            self.unknown_component = other.unknown_component;
        }
        if other.unknown_modal.is_some() {
            self.unknown_modal = other.unknown_modal;
        }
        self.message.extend(other.message);
        self.ready.extend(other.ready);
        self.event.extend(other.event);
        self.guild_create.extend(other.guild_create);
        self.member_join.extend(other.member_join);
        self.member_leave.extend(other.member_leave);
        self.reaction_add.extend(other.reaction_add);
        self.voice_state.extend(other.voice_state);
        self.before_command.extend(other.before_command);
        self.around_command.extend(other.around_command);
        self.scheduled.extend(other.scheduled);
        if other.on_error.is_some() {
            self.on_error = other.on_error;
        }
    }
}

/// State every context points to.
struct Shared {
    http: Client,
    /// Current shard manager; replaced when the bot reshards.
    manager: Arc<std::sync::Mutex<Arc<ShardManager>>>,
    #[cfg(feature = "cache")]
    cache: Option<Arc<cordex_cache::InMemoryCache>>,
    #[cfg(feature = "voice")]
    voice: Option<Arc<cordex_voice::VoiceManager>>,
    me: OnceLock<Id<UserMarker>>,
    handlers: Handlers,
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    error_message: Option<Box<str>>,
    cooldown_message: Option<CooldownMessage>,
    cooldown_tracker: Cooldowns<(Box<str>, u64)>,
    auto_defer: bool,
    standby: Standby,
    has_message_content_intent: bool,
    warned_empty_content: AtomicBool,
}

#[cfg(feature = "voice")]
struct BotVoiceGateway {
    manager: Arc<std::sync::Mutex<Arc<ShardManager>>>,
}

#[cfg(feature = "voice")]
impl cordex_voice::VoiceGateway for BotVoiceGateway {
    fn send_voice_state(
        &self,
        update: cordex_model::gateway::command::UpdateVoiceState,
    ) -> cordex_voice::gateway::BoxVoiceFuture<'_> {
        let manager = Arc::clone(
            &self
                .manager
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        Box::pin(async move {
            manager
                .update_voice_state(&update)
                .await
                .map_err(cordex_voice::VoiceError::from)
        })
    }
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
    shards: Option<(std::ops::Range<u32>, u32)>,
    commands: Vec<Command>,
    guild_commands: Option<Id<GuildMarker>>,
    register_commands: bool,
    handlers: Handlers,
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    error_message: Option<Box<str>>,
    cooldown_message: Option<CooldownMessage>,
    auto_defer: bool,
    default_allowed_mentions: Option<AllowedMentions>,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
    extra_events: EventTypeFlags,
    reshard_interval: Option<Duration>,
    #[cfg(feature = "cache")]
    cache: Option<cordex_cache::CacheConfig>,
    #[cfg(feature = "voice")]
    voice: bool,
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
            cooldown_message: None,
            auto_defer: true,
            default_allowed_mentions: None,
            presence: None,
            extra_events: EventTypeFlags::empty(),
            reshard_interval: Some(DEFAULT_RESHARD_INTERVAL),
            #[cfg(feature = "cache")]
            cache: None,
            #[cfg(feature = "voice")]
            voice: false,
        }
    }

    /// Create a bot by reading the `DISCORD_TOKEN` environment variable.
    ///
    /// Returns [`Error::MissingEnv`] with a clear hint when the variable is
    /// missing or empty.
    pub fn from_env() -> Result<Self> {
        Self::from_env_var("DISCORD_TOKEN")
    }

    /// Create a bot by reading the environment variable `name`.
    ///
    /// Returns [`Error::MissingEnv`] when the variable is missing or empty.
    pub fn from_env_var(name: &str) -> Result<Self> {
        let token = std::env::var(name)
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::MissingEnv(name.into()))?;
        Ok(Self::new(token))
    }

    /// Mount a [`BotModule`] (commands, component/modal/event handlers,
    /// intents, and module-scoped `before_command` guards).
    pub fn mount(mut self, module: BotModule) -> Self {
        let (intents, extra_events, commands, handlers) = module.finalize();
        self.intents |= intents;
        self.extra_events |= extra_events;
        module::merge_commands(&mut self.commands, commands);
        self.handlers.merge(handlers);
        self
    }

    /// Enable voice signaling ([`cordex_voice::VoiceManager`]): adds
    /// `Intents::GUILD_VOICE_STATES`, parses `VOICE_STATE_UPDATE` and
    /// `VOICE_SERVER_UPDATE`, feeds them into the manager, and exposes it
    /// via [`Context::voice`].
    #[cfg(feature = "voice")]
    pub fn voice(mut self) -> Self {
        self.voice = true;
        self.intents |= Intents::GUILD_VOICE_STATES;
        self
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
        let count = count.max(1);
        self.shards = Some((0..count, count));
        self
    }

    /// Run a specific `range` of `total` shards in this process (multi-process
    /// clustering; turns off resharding).
    pub fn shard_range(mut self, range: std::ops::Range<u32>, total: u32) -> Self {
        self.shards = Some((range, total));
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

    /// Customize the ephemeral message sent when a command registered with
    /// [`command_with_cooldown`](Self::command_with_cooldown) is on cooldown.
    pub fn cooldown_message<F>(mut self, format: F) -> Self
    where
        F: Fn(Duration) -> String + Send + Sync + 'static,
    {
        self.cooldown_message = Some(Arc::new(format));
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
    /// (for cooldowns, maintenance mode, blacklists, or logging). Multiple
    /// hooks run in registration order.
    pub fn before_command<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<bool>> + Send + 'static,
    {
        self.handlers
            .before_command
            .push(Arc::new(move |ctx| Box::pin(handler(ctx))));
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

    /// Register a command with a built-in per-command cooldown.
    ///
    /// When triggered while on cooldown, the bot replies ephemerally with the
    /// remaining wait time (customizable via
    /// [`cooldown_message`](Self::cooldown_message)) and skips the handler.
    pub fn command_with_cooldown<F, Fut>(
        mut self,
        command: Command,
        duration: Duration,
        scope: CooldownScope,
        handler: F,
    ) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let name: Box<str> = command.name().into();
        self.handlers
            .command_cooldowns
            .insert(name, cooldown::CommandCooldown { duration, scope });
        self.command(command, handler)
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

    /// Handler for any button or select menu whose `custom_id` starts with
    /// `prefix` (e.g. `"ticket_close_"`). The unmatched remainder is exposed
    /// via [`ComponentContext::matched_tail`]. Longest matching prefix wins.
    pub fn on_component_prefix<F, Fut>(mut self, prefix: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.component_prefixes.push(PrefixRoute {
            prefix: prefix.into(),
            handler: boxed(handler),
        });
        self
    }

    /// Alias of [`on_component_prefix`](Self::on_component_prefix) for buttons.
    pub fn on_button_prefix<F, Fut>(self, prefix: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.on_component_prefix(prefix, handler)
    }

    /// Handler for components whose `custom_id` matches a `{param}` segment
    /// pattern delimited by `:` or `/` (e.g. `"ticket:{action}:{id}"`).
    /// Captured values are read via [`ComponentContext::param`] and
    /// [`ComponentContext::param_id`].
    pub fn on_component_pattern<F, Fut>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .component_patterns
            .push(PatternRoute::parse(pattern, boxed(handler)));
        self
    }

    /// Alias of [`on_component_pattern`](Self::on_component_pattern) for buttons.
    pub fn on_button_pattern<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.on_component_pattern(pattern, handler)
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

    /// Handler for any submitted modal whose `custom_id` starts with `prefix`.
    /// Read the remainder with [`ModalContext::matched_tail`].
    pub fn on_modal_prefix<F, Fut>(mut self, prefix: &str, handler: F) -> Self
    where
        F: Fn(ModalContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.modal_prefixes.push(PrefixRoute {
            prefix: prefix.into(),
            handler: boxed(handler),
        });
        self
    }

    /// Handler for submitted modals matching a `{param}` segment pattern
    /// (e.g. `"report:{target_id}"`). Read captured values via
    /// [`ModalContext::param`] and [`ModalContext::param_id`].
    pub fn on_modal_pattern<F, Fut>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(ModalContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .modal_patterns
            .push(PatternRoute::parse(pattern, boxed(handler)));
        self
    }

    /// Fallback handler for buttons/selects whose `custom_id` matches no
    /// [`on_component`](Self::on_component) handler (e.g. old buttons from a
    /// previous bot version).
    pub fn on_unknown_component<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ComponentContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.unknown_component = Some(boxed(handler));
        self
    }

    /// Fallback handler for submitted modals whose `custom_id` matches no
    /// [`on_modal`](Self::on_modal) handler.
    pub fn on_unknown_modal<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ModalContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.unknown_modal = Some(boxed(handler));
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

    /// Handler for `GUILD_CREATE` (when a guild becomes available or the bot
    /// joins one). Adds `Intents::GUILDS`.
    pub fn on_guild_create<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Arc<cordex_model::gateway::event::GuildCreate>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILDS;
        self.handlers
            .guild_create
            .push(Arc::new(move |(ctx, ev)| Box::pin(handler(ctx, ev))));
        self
    }

    /// Handler for `GUILD_MEMBER_ADD`. Adds the privileged
    /// `Intents::GUILD_MEMBERS` intent (enable it in the Developer Portal).
    pub fn on_member_join<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Arc<cordex_model::Member>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_MEMBERS;
        self.handlers
            .member_join
            .push(Arc::new(move |(ctx, ev)| Box::pin(handler(ctx, ev))));
        self
    }

    /// Handler for `GUILD_MEMBER_REMOVE`. Adds the privileged
    /// `Intents::GUILD_MEMBERS` intent (enable it in the Developer Portal).
    pub fn on_member_leave<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Arc<cordex_model::gateway::event::GuildMemberRemove>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_MEMBERS;
        self.handlers
            .member_leave
            .push(Arc::new(move |(ctx, ev)| Box::pin(handler(ctx, ev))));
        self
    }

    /// Handler for `MESSAGE_REACTION_ADD`. Adds
    /// `Intents::GUILD_MESSAGE_REACTIONS` and
    /// `Intents::DIRECT_MESSAGE_REACTIONS`.
    pub fn on_reaction_add<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Arc<cordex_model::gateway::event::MessageReactionAdd>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_MESSAGE_REACTIONS | Intents::DIRECT_MESSAGE_REACTIONS;
        self.handlers
            .reaction_add
            .push(Arc::new(move |(ctx, ev)| Box::pin(handler(ctx, ev))));
        self
    }

    /// Handler for `VOICE_STATE_UPDATE`. Adds `Intents::GUILD_VOICE_STATES`.
    pub fn on_voice_state<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(Context, Arc<cordex_model::voice::VoiceState>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_VOICE_STATES;
        self.handlers
            .voice_state
            .push(Arc::new(move |(ctx, ev)| Box::pin(handler(ctx, ev))));
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
            && h.component_prefixes.is_empty()
            && h.component_patterns.is_empty()
            && h.modals.is_empty()
            && h.modal_prefixes.is_empty()
            && h.modal_patterns.is_empty()
            && h.unknown_component.is_none()
            && h.unknown_modal.is_none())
        {
            names.push("INTERACTION_CREATE");
        }
        if !h.message.is_empty() {
            names.push("MESSAGE_CREATE");
        }
        if !h.guild_create.is_empty() {
            names.push("GUILD_CREATE");
        }
        if !h.member_join.is_empty() {
            names.push("GUILD_MEMBER_ADD");
        }
        if !h.member_leave.is_empty() {
            names.push("GUILD_MEMBER_REMOVE");
        }
        if !h.reaction_add.is_empty() {
            names.push("MESSAGE_REACTION_ADD");
        }
        if !h.voice_state.is_empty() {
            names.push("VOICE_STATE_UPDATE");
        }
        #[cfg(feature = "cache")]
        if let Some(config) = &self.cache {
            names.extend(config.wanted_events());
        }
        #[cfg(feature = "voice")]
        if self.voice {
            names.push("VOICE_STATE_UPDATE");
            names.push("VOICE_SERVER_UPDATE");
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
        let fixed_shards = self.shards.clone();
        let presence = self.presence.clone();
        let mut manager = build_manager(
            &self.token,
            self.intents,
            &http,
            event_types,
            fixed_shards.clone(),
            presence.clone(),
        )
        .await?;
        let mut events = manager.events();
        let token = self.token.clone();
        let intents = self.intents;
        let reshard_interval = self.reshard_interval;
        let guild_commands = self.guild_commands;
        let registered = AtomicBool::new(!self.register_commands);
        let (shared, commands) = self.build_shared(http, manager);

        shared.manager().start().await?;

        // Resharding: a task prepares a new manager and hands it over here.
        let (switch_tx, mut switch_rx) = tokio::sync::mpsc::channel::<Reshard>(1);
        if fixed_shards.is_none()
            && let Some(interval) = reshard_interval
        {
            tokio::spawn(reshard_loop(
                Arc::clone(&shared),
                token,
                intents,
                event_types,
                presence,
                interval,
                switch_tx,
            ));
        }

        let mut schedulers: Option<SchedulerHandle> = None;
        let mut shutdown = std::pin::pin!(shutdown);
        loop {
            let item = tokio::select! {
                () = &mut shutdown => {
                    tracing::info!("shutdown signal received; closing shards");
                    if let Some(handle) = schedulers.take() {
                        handle.shutdown(Duration::from_secs(5)).await;
                    }
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
            if schedulers.is_none()
                && !shared.handlers.scheduled.is_empty()
                && matches!(
                    item,
                    ManagerEvent::Dispatch {
                        event: Event::Ready(_),
                        ..
                    }
                )
            {
                schedulers = Some(scheduler::spawn_all(&shared, &shared.handlers.scheduled));
            }
            on_manager_event(&shared, item, &registered, guild_commands, &commands).await?;
        }
        if let Some(handle) = schedulers.take() {
            handle.shutdown(Duration::from_secs(5)).await;
        }
        Ok(())
    }

    /// Register a periodic task that runs every `interval` after the first `READY`.
    ///
    /// Uses `MissedTickBehavior::Skip` and isolates errors/panics per tick.
    pub fn every<F, Fut>(mut self, interval: Duration, f: F) -> Self
    where
        F: Fn(Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .scheduled
            .push(scheduler::ScheduledTask::Periodic {
                interval,
                immediate: false,
                handler: boxed(f),
            });
        self
    }

    /// Like [`every`](Self::every), but runs the first tick immediately on `READY`.
    pub fn every_immediate<F, Fut>(mut self, interval: Duration, f: F) -> Self
    where
        F: Fn(Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .scheduled
            .push(scheduler::ScheduledTask::Periodic {
                interval,
                immediate: true,
                handler: boxed(f),
            });
        self
    }

    /// Register a long-running background task started on the first `READY`.
    ///
    /// The task receives a [`ShutdownToken`] that is cancelled when the bot shuts down.
    pub fn background<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Context, ShutdownToken) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .scheduled
            .push(scheduler::ScheduledTask::Background {
                handler: Arc::new(move |ctx, token| Box::pin(f(ctx, token))),
            });
        self
    }

    /// Register an `around` middleware that wraps every command handler.
    ///
    /// Middlewares run in registration order (first registered is outermost).
    /// Call `next.run(ctx).await` to continue to the next middleware or command.
    pub fn around_command<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(CommandContext, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .around_command
            .push(Arc::new(move |ctx, next| Box::pin(f(ctx, next))));
        self
    }

    /// If `DISCORD_DEV_GUILD_ID` is set in the environment to a valid guild
    /// snowflake, register commands in that guild instead of globally for
    /// instant iteration during development.
    pub fn sync_dev_guild_from_env(mut self) -> Self {
        if let Some(guild_id) = cordex_interactions::sync::dev_guild_id_from_env() {
            self.guild_commands = Some(guild_id);
        }
        self
    }

    fn build_shared(
        mut self,
        http: Client,
        manager: ShardManager,
    ) -> (Arc<Shared>, Arc<[Command]>) {
        if !self.handlers.around_command.is_empty() {
            let mws = std::mem::take(&mut self.handlers.around_command);
            for handler in self.handlers.commands.values_mut() {
                *handler = middleware::wrap_around(Arc::clone(handler), &mws);
            }
        }
        let manager = Arc::new(std::sync::Mutex::new(Arc::new(manager)));
        #[cfg(feature = "voice")]
        let voice = self.voice.then(|| {
            Arc::new(cordex_voice::VoiceManager::new(Arc::new(BotVoiceGateway {
                manager: Arc::clone(&manager),
            })))
        });

        let shared = Arc::new(Shared {
            http,
            manager,
            #[cfg(feature = "cache")]
            cache: self
                .cache
                .map(|config| Arc::new(cordex_cache::InMemoryCache::new(config))),
            #[cfg(feature = "voice")]
            voice,
            me: OnceLock::new(),
            handlers: self.handlers,
            data: self.data,
            error_message: self.error_message,
            cooldown_message: self.cooldown_message,
            cooldown_tracker: Cooldowns::new(),
            auto_defer: self.auto_defer,
            standby: Standby::new(),
            has_message_content_intent: self.intents.contains(Intents::MESSAGE_CONTENT),
            warned_empty_content: AtomicBool::new(false),
        });
        (shared, self.commands.into())
    }

    /// Build an offline handle for driving this bot without connecting to
    /// Discord. Used by `cordex-test` to inject events and inspect state.
    pub fn into_offline(self, http: Client, manager: ShardManager) -> OfflineBot {
        let (shared, commands) = self.build_shared(http, manager);
        OfflineBot { shared, commands }
    }
}

/// Offline bot driver produced by [`Bot::into_offline`].
///
/// Dispatches gateway events directly through the bot's cache, standby, and
/// registered handlers without opening any network connections.
pub struct OfflineBot {
    shared: Arc<Shared>,
    commands: Arc<[Command]>,
}

impl OfflineBot {
    /// Dispatch one gateway event as if it arrived on `shard_id`.
    pub fn dispatch(&self, shard_id: ShardId, event: Event) {
        dispatch(&self.shared, shard_id, event);
    }

    /// Build a [`Context`] bound to shard 0.
    pub fn context(&self) -> Context {
        Context {
            shared: Arc::clone(&self.shared),
            shard: ShardId::one(),
        }
    }

    /// Underlying HTTP client.
    pub fn http(&self) -> &Client {
        &self.shared.http
    }

    /// Registered application commands.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// In-memory cache, if enabled on the [`Bot`].
    #[cfg(feature = "cache")]
    pub fn cache(&self) -> Option<&Arc<cordex_cache::InMemoryCache>> {
        self.shared.cache.as_ref()
    }

    /// Underlying [`Standby`] event waiter registry.
    pub fn standby(&self) -> &Standby {
        &self.shared.standby
    }

    /// Start all registered [`Bot::every`] / [`Bot::background`] tasks for
    /// offline testing. Call [`SchedulerHandle::shutdown`] to stop them.
    pub fn start_schedulers(&self) -> SchedulerHandle {
        scheduler::spawn_all(&self.shared, &self.shared.handlers.scheduled)
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
    fixed_shards: Option<(std::ops::Range<u32>, u32)>,
    presence: Option<cordex_model::gateway::presence::UpdatePresence>,
) -> Result<ShardManager> {
    let mut builder = ShardManager::builder(token, intents)
        .http(http.clone())
        .event_types(event_types);
    if let Some(presence) = presence {
        builder = builder.presence(presence);
    }
    let builder = match fixed_shards {
        Some((range, total)) => builder.shards(range, total),
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
    let previous = shared
        .cache
        .as_ref()
        .map_or(cordex_cache::PreviousState::None, |cache| {
            cache.update_with_previous(&event)
        });
    #[cfg(feature = "voice")]
    if let Some(voice) = &shared.voice {
        voice.update_from_event(&event);
    }
    let taken = shared.standby.process(&event) > 0;
    // A component or modal interaction a waiter took (`ctx.wait_for_button` /
    // `ctx.wait_for_modal`) is answered there; routing it to `on_component` /
    // `on_modal` too would answer twice.
    if taken
        && let Event::InteractionCreate(interaction) = &event
        && matches!(
            interaction.kind,
            InteractionType::MessageComponent | InteractionType::ModalSubmit
        )
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
    #[cfg(feature = "cache")]
    let previous = Arc::new(previous);
    for handler in &handlers.event {
        spawn_guarded(
            "event",
            ctx.clone(),
            handler(EventContext {
                ctx: ctx.clone(),
                event: Arc::clone(&event),
                #[cfg(feature = "cache")]
                previous: Arc::clone(&previous),
            }),
        );
    }
    match &*event {
        Event::Ready(ready) => route_ready(&ctx, || Arc::new(Ready::clone(ready)), ready.user.id),
        Event::MessageCreate(message) => route_message(&ctx, || Arc::new(Message::clone(message))),
        Event::InteractionCreate(interaction) => route_interaction(ctx, interaction.clone()),
        Event::GuildCreate(g) => route_typed(&ctx, &handlers.guild_create, "guild_create", || {
            Arc::new(g.clone())
        }),
        Event::GuildMemberAdd(m) => route_typed(&ctx, &handlers.member_join, "member_join", || {
            Arc::new((**m).clone())
        }),
        Event::GuildMemberRemove(r) => {
            route_typed(&ctx, &handlers.member_leave, "member_leave", || {
                Arc::new((**r).clone())
            });
        }
        Event::MessageReactionAdd(r) => {
            route_typed(&ctx, &handlers.reaction_add, "reaction_add", || {
                Arc::new((**r).clone())
            });
        }
        Event::VoiceStateUpdate(v) => {
            route_typed(&ctx, &handlers.voice_state, "voice_state", || {
                Arc::new((**v).clone())
            });
        }
        _ => {}
    }
}

fn route_owned(ctx: Context, event: Event) {
    let handlers = &ctx.shared.handlers;
    match event {
        Event::Ready(ready) => {
            let id = ready.user.id;
            route_ready(&ctx, || Arc::from(ready), id);
        }
        Event::MessageCreate(message) => route_message(&ctx, || Arc::from(message)),
        Event::InteractionCreate(interaction) => route_interaction(ctx, interaction),
        Event::GuildCreate(g) => {
            route_typed(&ctx, &handlers.guild_create, "guild_create", || {
                Arc::from(g)
            });
        }
        Event::GuildMemberAdd(m) => {
            route_typed(&ctx, &handlers.member_join, "member_join", || Arc::from(m));
        }
        Event::GuildMemberRemove(r) => {
            route_typed(&ctx, &handlers.member_leave, "member_leave", || {
                Arc::from(r)
            });
        }
        Event::MessageReactionAdd(r) => {
            route_typed(&ctx, &handlers.reaction_add, "reaction_add", || {
                Arc::from(r)
            });
        }
        Event::VoiceStateUpdate(v) => {
            route_typed(&ctx, &handlers.voice_state, "voice_state", || Arc::from(v));
        }
        _ => {}
    }
}

fn route_typed<T>(
    ctx: &Context,
    handlers: &[Handler<(Context, Arc<T>)>],
    kind: &'static str,
    payload: impl FnOnce() -> Arc<T>,
) {
    if handlers.is_empty() {
        return;
    }
    let payload = payload();
    for handler in handlers {
        spawn_guarded(
            kind,
            ctx.clone(),
            handler((ctx.clone(), Arc::clone(&payload))),
        );
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
    if !ctx.shared.has_message_content_intent
        && message.guild_id.is_some()
        && !message.author.bot
        && message.content.is_empty()
        && message.attachments.is_empty()
        && message.embeds.is_empty()
        && !ctx
            .shared
            .warned_empty_content
            .swap(true, Ordering::Relaxed)
    {
        tracing::warn!(
            channel_id = %message.channel_id,
            "received a guild message with empty content because MESSAGE_CONTENT is not enabled; \
             call Bot::message_content() and turn on Message Content Intent in the Discord Developer Portal"
        );
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

struct MatchedRoute<C> {
    handler: Handler<C>,
    matched_tail: Box<str>,
    params: RouteParams,
}

/// Look up a component/modal handler:
/// 1. Exact id in `map`, or longest `:`-delimited prefix in `map`
///    (e.g. `"giveaway:join:42"` checks `"giveaway:join:42"`, `"giveaway:join"`, `"giveaway"`).
/// 2. Segment pattern in `patterns` (`"ticket:{action}:{id}"`; highest literal segment count wins).
/// 3. General string prefix in `prefixes` (`"ticket_close_"`; longest prefix wins).
fn lookup_route<C>(
    map: &HashMap<Box<str>, Handler<C>>,
    patterns: &[PatternRoute<C>],
    prefixes: &[PrefixRoute<C>],
    id: &str,
) -> Option<MatchedRoute<C>> {
    if let Some(handler) = map.get(id) {
        return Some(MatchedRoute {
            handler: Arc::clone(handler),
            matched_tail: Box::from(""),
            params: Box::from([]),
        });
    }
    let mut prefix = id;
    while let Some((head, _)) = prefix.rsplit_once(':') {
        if let Some(handler) = map.get(head) {
            let tail = id
                .strip_prefix(head)
                .and_then(|rest| rest.strip_prefix(':'))
                .unwrap_or("");
            return Some(MatchedRoute {
                handler: Arc::clone(handler),
                matched_tail: Box::from(tail),
                params: Box::from([]),
            });
        }
        prefix = head;
    }

    let mut best_pattern: Option<(usize, &PatternRoute<C>, RouteParams)> = None;
    for route in patterns {
        if let Some(params) = route.matches(id)
            && best_pattern
                .as_ref()
                .is_none_or(|(best_lit, _, _)| route.literal_count > *best_lit)
        {
            best_pattern = Some((route.literal_count, route, params));
        }
    }
    if let Some((_, route, params)) = best_pattern {
        return Some(MatchedRoute {
            handler: Arc::clone(&route.handler),
            matched_tail: Box::from(""),
            params,
        });
    }

    let mut best_prefix: Option<&PrefixRoute<C>> = None;
    for route in prefixes {
        if id.starts_with(&*route.prefix)
            && best_prefix.is_none_or(|prev| route.prefix.len() > prev.prefix.len())
        {
            best_prefix = Some(route);
        }
    }
    if let Some(route) = best_prefix {
        let tail = id.strip_prefix(&*route.prefix).unwrap_or("");
        return Some(MatchedRoute {
            handler: Arc::clone(&route.handler),
            matched_tail: Box::from(tail),
            params: Box::from([]),
        });
    }

    None
}

fn cooldown_scope_key(ctx: &CommandContext, scope: CooldownScope) -> u64 {
    let user_id = ctx.user().map_or(0, |u| u.id.get());
    match scope {
        CooldownScope::User => user_id,
        CooldownScope::Channel => ctx.channel_id().map_or(user_id, Id::get),
        CooldownScope::Guild => ctx.guild_id().map_or(user_id, Id::get),
        CooldownScope::Global => 0,
    }
}

fn format_cooldown_wait(custom: Option<&CooldownMessage>, remaining: Duration) -> String {
    if let Some(formatter) = custom {
        return formatter(remaining);
    }
    let secs = remaining.as_secs().max(1);
    format!("This command is on cooldown. Try again in {secs}s.")
}

fn prepare_command_future(
    ctx: Context,
    interaction: Box<Interaction>,
    data: cordex_model::interaction::CommandData,
    handle: &InteractionHandle,
) -> Option<BoxFuture> {
    let handlers = &ctx.shared.handlers;
    let Some(handler) = handlers.commands.get(&*data.name).cloned() else {
        tracing::warn!(command = %data.name, "no handler for command");
        let handle = handle.clone();
        tokio::spawn(async move {
            let reply = Reply::new("This command is not available.").ephemeral();
            let _ = handle.respond(reply).await;
        });
        return None;
    };
    let cooldown = handlers.command_cooldowns.get(&*data.name).copied();
    let before: Arc<[BeforeCommand]> = handlers.before_command.clone().into();
    let auto_defer = ctx.shared.auto_defer;
    let cmd_ctx = CommandContext {
        ctx,
        interaction,
        data,
        handle: handle.clone(),
    };
    if let Some(cd) = cooldown {
        let scope_key = cooldown_scope_key(&cmd_ctx, cd.scope);
        let key = (Box::<str>::from(cmd_ctx.name()), scope_key);
        if let Some(wait) = cmd_ctx.shared.cooldown_tracker.check(key, cd.duration) {
            let msg = format_cooldown_wait(cmd_ctx.shared.cooldown_message.as_ref(), wait);
            let handle = handle.clone();
            tokio::spawn(async move {
                let _ = handle.respond(Reply::new(msg).ephemeral()).await;
            });
            return None;
        }
    }
    if auto_defer {
        handle.auto_defer(DEFAULT_AUTO_DEFER, false);
    }
    Some(if before.is_empty() {
        handler(cmd_ctx)
    } else {
        wrap_with_guards(handler, before)(cmd_ctx)
    })
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
            let Some(fut) = prepare_command_future(ctx, interaction, data, &handle) else {
                return;
            };
            fut
        }
        Some(InteractionData::MessageComponent(data)) => {
            let matched = lookup_route(
                &handlers.components,
                &handlers.component_patterns,
                &handlers.component_prefixes,
                &data.custom_id,
            )
            .or_else(|| {
                handlers
                    .unknown_component
                    .as_ref()
                    .map(|handler| MatchedRoute {
                        handler: Arc::clone(handler),
                        matched_tail: Box::from(""),
                        params: Box::from([]),
                    })
            });
            let Some(matched) = matched else {
                tracing::debug!(custom_id = %data.custom_id, "no component handler");
                return;
            };
            (matched.handler)(ComponentContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
                matched_tail: matched.matched_tail,
                params: matched.params,
            })
        }
        Some(InteractionData::ModalSubmit(data)) => {
            let matched = lookup_route(
                &handlers.modals,
                &handlers.modal_patterns,
                &handlers.modal_prefixes,
                &data.custom_id,
            )
            .or_else(|| {
                handlers.unknown_modal.as_ref().map(|handler| MatchedRoute {
                    handler: Arc::clone(handler),
                    matched_tail: Box::from(""),
                    params: Box::from([]),
                })
            });
            let Some(matched) = matched else {
                tracing::debug!(custom_id = %data.custom_id, "no modal handler");
                return;
            };
            if auto_defer {
                handle.auto_defer(DEFAULT_AUTO_DEFER, false);
            }
            (matched.handler)(ModalContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
                matched_tail: matched.matched_tail,
                params: matched.params,
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
