//! Composable modules for organizing larger bots into separate files.
//!
//! Build a [`BotModule`] in each feature file (e.g. `commands/moderation.rs`)
//! and attach it to [`Bot`](super::Bot) with [`Bot::mount`](super::Bot::mount):
//!
//! ```no_run
//! use cordex::prelude::*;
//!
//! fn moderation() -> BotModule {
//!     BotModule::new()
//!         .before_command(|ctx: CommandContext| async move {
//!             if !ctx.user_can(Permissions::MODERATE_MEMBERS) {
//!                 ctx.reply_ephemeral("Moderators only.").await?;
//!                 return Ok(false);
//!             }
//!             Ok(true)
//!         })
//!         .slash("warn", "Warn a member", |ctx: CommandContext| async move {
//!             ctx.reply_ephemeral("Warned.").await
//!         })
//! }
//!
//! # fn wire(token: String) -> Bot {
//! Bot::new(token).mount(moderation())
//! # }
//! ```

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use cordex_gateway::EventTypeFlags;
use cordex_interactions::Command;
use cordex_model::gateway::Intents;

use super::cooldown::{CommandCooldown, CooldownScope};
use super::{
    AutocompleteContext, BeforeCommand, CommandContext, ComponentContext, Context, EventContext,
    Handlers, MessageContext, ModalContext, PatternRoute, PrefixRoute, ReadyContext, boxed,
    wrap_with_guards,
};
use crate::Result;

/// A reusable collection of commands, component/modal handlers, event handlers,
/// and module-scoped `before_command` guards that can be mounted onto a
/// [`Bot`](super::Bot) or another [`BotModule`].
#[must_use = "a BotModule does nothing until mounted with `Bot::mount`"]
#[derive(Default)]
pub struct BotModule {
    pub(super) intents: Intents,
    pub(super) extra_events: EventTypeFlags,
    pub(super) commands: Vec<Command>,
    pub(super) handlers: Handlers,
    /// Guards scoped strictly to commands registered in this module (and any
    /// sub-modules mounted into it).
    pub(super) local_before_command: Vec<BeforeCommand>,
    /// `around_command` middlewares scoped strictly to commands in this module.
    pub(super) local_around_command: Vec<super::middleware::AroundCommand>,
}

impl std::fmt::Debug for BotModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotModule")
            .field("intents", &self.intents)
            .field("commands", &self.commands.len())
            .finish_non_exhaustive()
    }
}

impl BotModule {
    /// Empty module.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add gateway intents required by handlers in this module.
    pub fn intents(mut self, intents: Intents) -> Self {
        self.intents |= intents;
        self
    }

    /// Extra gateway events to parse for [`Context::wait_for`].
    pub fn parse_events(mut self, events: EventTypeFlags) -> Self {
        self.extra_events |= events;
        self
    }

    /// Guard run before every command registered in **this module** (and any
    /// nested sub-modules mounted into it). Does not affect commands registered
    /// outside this module.
    pub fn before_command<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<bool>> + Send + 'static,
    {
        self.local_before_command
            .push(Arc::new(move |ctx| Box::pin(handler(ctx))));
        self
    }

    /// Register a command and its handler.
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
            .insert(name, CommandCooldown { duration, scope });
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

    /// Autocomplete handler for `command`.
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

    /// Handler for buttons and select menus with this `custom_id` (or `:` prefix).
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

    /// Handler for any component whose `custom_id` starts with `prefix`.
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

    /// Handler for components matching a `{param}` segment pattern
    /// (e.g. `"ticket:{action}:{id}"`).
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

    /// Handler for submitted modals with this `custom_id` (or `:` prefix).
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

    /// Handler for any modal whose `custom_id` starts with `prefix`.
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

    /// Handler for modals matching a `{param}` segment pattern.
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

    /// Handler for new messages. Adds `GUILD_MESSAGES | DIRECT_MESSAGES`.
    pub fn on_message<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(MessageContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.intents |= Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES;
        self.handlers.message.push(boxed(handler));
        self
    }

    /// Handler called on every `READY`.
    pub fn on_ready<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ReadyContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.ready.push(boxed(handler));
        self
    }

    /// Handler for every gateway event.
    pub fn on_event<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(EventContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers.event.push(boxed(handler));
        self
    }

    /// Handler for `GUILD_CREATE`. Adds `Intents::GUILDS`.
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

    /// Handler for `GUILD_MEMBER_ADD`. Adds `Intents::GUILD_MEMBERS`.
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

    /// Handler for `GUILD_MEMBER_REMOVE`. Adds `Intents::GUILD_MEMBERS`.
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

    /// Handler for `MESSAGE_REACTION_ADD`. Adds reaction intents.
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

    /// Nest another [`BotModule`] inside this module.
    pub fn mount(mut self, child: Self) -> Self {
        let (intents, extra_events, commands, handlers) = child.finalize();
        self.intents |= intents;
        self.extra_events |= extra_events;
        merge_commands(&mut self.commands, commands);
        self.handlers.merge(handlers);
        self
    }

    /// Register a periodic task that runs every `interval` after the first `READY`.
    pub fn every<F, Fut>(mut self, interval: Duration, f: F) -> Self
    where
        F: Fn(Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .scheduled
            .push(super::scheduler::ScheduledTask::Periodic {
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
            .push(super::scheduler::ScheduledTask::Periodic {
                interval,
                immediate: true,
                handler: boxed(f),
            });
        self
    }

    /// Register a long-running background task started on the first `READY`.
    pub fn background<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(Context, super::ShutdownToken) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.handlers
            .scheduled
            .push(super::scheduler::ScheduledTask::Background {
                handler: Arc::new(move |ctx, token| Box::pin(f(ctx, token))),
            });
        self
    }

    /// Register an `around` middleware scoped strictly to commands in this module.
    pub fn around_command<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(CommandContext, super::Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.local_around_command
            .push(Arc::new(move |ctx, next| Box::pin(f(ctx, next))));
        self
    }

    /// Wrap this module's command handlers with its `local_before_command`
    /// guards and `local_around_command` middlewares, and return its parts for mounting.
    pub(super) fn finalize(mut self) -> (Intents, EventTypeFlags, Vec<Command>, Handlers) {
        if !self.local_before_command.is_empty() {
            let guards: Arc<[BeforeCommand]> = self.local_before_command.into();
            for handler in self.handlers.commands.values_mut() {
                *handler = wrap_with_guards(Arc::clone(handler), Arc::clone(&guards));
            }
        }
        if !self.local_around_command.is_empty() {
            let mws = self.local_around_command;
            for handler in self.handlers.commands.values_mut() {
                *handler = super::middleware::wrap_around(Arc::clone(handler), &mws);
            }
        }
        (
            self.intents,
            self.extra_events,
            self.commands,
            self.handlers,
        )
    }
}

pub(super) fn merge_commands(target: &mut Vec<Command>, incoming: Vec<Command>) {
    for cmd in incoming {
        if target.iter().any(|existing| existing.name() == cmd.name()) {
            tracing::warn!(
                command = cmd.name(),
                "mounted module overrides an already registered command"
            );
            target.retain(|existing| existing.name() != cmd.name());
        }
        target.push(cmd);
    }
}
