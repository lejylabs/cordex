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
//! - **Intents** follow from the handlers (`GUILDS` always, message
//!   intents with [`on_message`](Bot::on_message)). Privileged ones only
//!   when asked for ([`message_content`](Bot::message_content),
//!   [`intents`](Bot::intents)).
//! - **Shards** are automatic (Discord's recommendation); reconnects and
//!   resumes happen inside the gateway.
//! - **Commands** are registered with one bulk overwrite on the first
//!   `READY` (global, or per guild with [`guild_commands`](Bot::guild_commands)).
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

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use cordex_gateway::{CloseMode, ManagerEvent, ShardId, ShardManager};
use cordex_http::Client;
use cordex_interactions::handle::{DEFAULT_AUTO_DEFER, ResponseState};
use cordex_interactions::{Command, InteractionHandle, Reply};
use cordex_model::gateway::Intents;
use cordex_model::gateway::event::Event;
use cordex_model::id::Id;
use cordex_model::id::marker::{ApplicationMarker, GuildMarker, UserMarker};
use cordex_model::interaction::{Interaction, InteractionData, InteractionType};

pub use context::{
    AutocompleteContext, CommandContext, ComponentContext, Context, EventContext, MessageContext,
    ModalContext, ReadyContext,
};

use crate::{Error, Result};

type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type Handler<C> = Arc<dyn Fn(C) -> BoxFuture + Send + Sync>;

fn boxed<C, F, Fut>(f: F) -> Handler<C>
where
    F: Fn(C) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |ctx| Box::pin(f(ctx)))
}

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
}

/// State every context points to.
struct Shared {
    http: Client,
    manager: ShardManager,
    #[cfg(feature = "cache")]
    cache: Option<Arc<cordex_cache::InMemoryCache>>,
    me: OnceLock<Id<UserMarker>>,
    handlers: Handlers,
    error_message: Option<Box<str>>,
    auto_defer: bool,
}

/// Builder and runner of a bot; see the [module docs](self).
#[must_use = "a Bot does nothing until `.run().await`"]
pub struct Bot {
    token: String,
    intents: Intents,
    shards: Option<u32>,
    commands: Vec<Command>,
    guild_commands: Option<Id<GuildMarker>>,
    handlers: Handlers,
    error_message: Option<Box<str>>,
    auto_defer: bool,
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
            intents: Intents::GUILDS,
            shards: None,
            commands: Vec::new(),
            guild_commands: None,
            handlers: Handlers::default(),
            error_message: Some(DEFAULT_ERROR_MESSAGE.into()),
            auto_defer: true,
            #[cfg(feature = "cache")]
            cache: None,
        }
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

    /// Fixed shard count instead of Discord's recommendation.
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
    pub fn cache(mut self, config: cordex_cache::CacheConfig) -> Self {
        self.cache = Some(config);
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

    /// Check the commands, connect and handle events until a shard fails
    /// fatally (bad token, disallowed intents, …); that error is returned.
    pub async fn run(self) -> Result<()> {
        for command in &self.commands {
            command.validate()?;
        }
        if !self.handlers.message.is_empty() && !self.intents.contains(Intents::MESSAGE_CONTENT) {
            tracing::info!(
                "on_message without MESSAGE_CONTENT: guild message text is empty unless the bot \
                 is mentioned; call Bot::message_content() to turn it on"
            );
        }

        let http = Client::new(&self.token)?;
        let builder = ShardManager::builder(&self.token, self.intents).http(http.clone());
        let builder = match self.shards {
            Some(n) => builder.shards(0..n, n),
            None => builder.automatic(),
        };
        let mut manager = builder.build().await?;
        let mut events = manager.events();

        let shared = Arc::new(Shared {
            http,
            manager,
            #[cfg(feature = "cache")]
            cache: self
                .cache
                .map(|config| Arc::new(cordex_cache::InMemoryCache::new(config))),
            me: OnceLock::new(),
            handlers: self.handlers,
            error_message: self.error_message,
            auto_defer: self.auto_defer,
        });
        let commands: Arc<[Command]> = self.commands.into();
        let guild_commands = self.guild_commands;
        let registered = AtomicBool::new(false);

        shared.manager.start().await?;

        while let Some(item) = events.recv().await {
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
                            Arc::clone(&commands),
                        ));
                    }
                    dispatch(&shared, shard, event);
                }
                ManagerEvent::Error { shard, error } => {
                    tracing::error!(shard = shard.number(), %error, "shard failed, stopping the bot");
                    shared.manager.shutdown(CloseMode::Terminate).await;
                    return Err(Error::Gateway(error));
                }
                ManagerEvent::RecommendedShardCount {
                    running,
                    recommended,
                } => {
                    tracing::warn!(
                        running,
                        recommended,
                        "Discord recommends more shards; restart the bot to use them"
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// One bulk overwrite with all commands (only when there are any, so a
/// bot without commands never wipes commands set elsewhere).
async fn register_commands(
    http: Client,
    app: Id<ApplicationMarker>,
    guild: Option<Id<GuildMarker>>,
    commands: Arc<[Command]>,
) {
    if commands.is_empty() {
        return;
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
    let handlers = &shared.handlers;
    if !handlers.event.is_empty() {
        let event = Arc::new(event.clone());
        for handler in &handlers.event {
            spawn_guarded(
                "event",
                handler(EventContext {
                    ctx: ctx.clone(),
                    event: Arc::clone(&event),
                }),
            );
        }
    }
    match event {
        Event::Ready(ready) => {
            let _ = shared.me.set(ready.user.id);
            let ready = Arc::from(ready);
            for handler in &handlers.ready {
                spawn_guarded(
                    "ready",
                    handler(ReadyContext {
                        ctx: ctx.clone(),
                        ready: Arc::clone(&ready),
                    }),
                );
            }
        }
        Event::MessageCreate(message) if !handlers.message.is_empty() => {
            if shared.me.get() == Some(&message.author.id) {
                return;
            }
            let message = Arc::from(message);
            for handler in &handlers.message {
                spawn_guarded(
                    "message",
                    handler(MessageContext {
                        ctx: ctx.clone(),
                        message: Arc::clone(&message),
                    }),
                );
            }
        }
        Event::InteractionCreate(interaction) => route_interaction(ctx, interaction),
        _ => {}
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
    let future = match interaction.data.take() {
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
            handler(CommandContext {
                ctx,
                interaction,
                data,
                handle: handle.clone(),
            })
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
        if !guarded("interaction", future).await
            && let Some(text) = error_message
        {
            answer_error(&handle, &text).await;
        }
    });
}

/// Spawn a handler; errors and panics are logged.
fn spawn_guarded(what: &'static str, future: BoxFuture) {
    tokio::spawn(guarded(what, future));
}

/// Await a handler in its own task; `true` when it returned `Ok`.
async fn guarded(what: &'static str, future: BoxFuture) -> bool {
    match tokio::spawn(future).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "{what} handler failed");
            false
        }
        Err(join) if join.is_panic() => {
            tracing::error!("{what} handler panicked");
            false
        }
        Err(_) => false,
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
