//! Handler contexts: what a handler gets and how it answers.

use std::fmt;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use cordex_gateway::{ShardId, ShardManager};
use cordex_http::Client;
use cordex_interactions::handle::ResponseState;
use cordex_interactions::options::ResolvedUser;
use cordex_interactions::{Choice, CommandOptions, InteractionHandle, Modal, Reply};
use cordex_model::gateway::event::{Event, MessageReactionAdd, Ready};
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker};
use cordex_model::interaction::{
    CommandData, CommandOptionValue, Interaction, InteractionData, InteractionType,
    MessageComponentData, ModalSubmitData, SubmittedComponent, SubmittedValue,
};
use cordex_model::message::Attachment;
use cordex_model::{Channel, Message, Role, User};

use super::Shared;
use crate::Result;

/// Shared part of every handler context: REST client, shards, cache.
///
/// Every specific context (`CommandContext`, `MessageContext`, …)
/// dereferences to this, so `ctx.http()` works everywhere.
#[derive(Clone)]
pub struct Context {
    pub(super) shared: Arc<Shared>,
    pub(super) shard: ShardId,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("shard", &self.shard)
            .finish_non_exhaustive()
    }
}

impl Context {
    /// REST client for everything the contexts do not wrap.
    pub fn http(&self) -> &Client {
        &self.shared.http
    }

    /// The shard manager (latency, status, restarts).
    pub fn shards(&self) -> Arc<ShardManager> {
        self.shared.manager()
    }

    /// Shard that received the event.
    pub fn shard(&self) -> ShardId {
        self.shard
    }

    /// The bot's own user id (known after the first `READY`).
    pub fn current_user_id(&self) -> Option<Id<UserMarker>> {
        self.shared.me.get().copied()
    }

    /// Shared state registered with [`Bot::data`](super::Bot::data).
    pub fn data<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.shared
            .data
            .get(&std::any::TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
    }

    /// Send a message (plain text, embed, or Components V2 [`Reply`]) to
    /// `channel`.
    pub async fn send(
        &self,
        channel: Id<ChannelMarker>,
        message: impl Into<Reply>,
    ) -> Result<Message> {
        let reply = message.into();
        let mut req = self.http().create_message(channel);
        if let Some(content) = reply.content_ref() {
            req = req.content(content.to_owned())?;
        }
        if !reply.embeds_ref().is_empty() {
            req = req.embeds(reply.embeds_ref().to_vec())?;
        }
        let mut flags = reply.flags_value();
        if let Some(components) = reply.components_ref() {
            if cordex_model::component::needs_components_v2(components) {
                flags |= cordex_model::message::MessageFlags::IS_COMPONENTS_V2;
            }
            req = req.components(components.to_vec());
        }
        if let Some(mentions) = reply.allowed_mentions_ref() {
            req = req.allowed_mentions(mentions.clone());
        }
        if !flags.is_empty() {
            req = req.flags(flags);
        }
        if reply.is_tts() {
            req = req.tts(true);
        }
        Ok(req.await?)
    }

    /// Open a DM channel with `user` and send a message (plain text, embed, or
    /// Components V2 [`Reply`]).
    pub async fn dm(&self, user: Id<UserMarker>, message: impl Into<Reply>) -> Result<Message> {
        let channel = self.http().create_dm(user).await?;
        self.send(channel.id, message).await
    }

    /// The cache, if the bot was built with [`Bot::cache`](super::Bot::cache).
    #[cfg(feature = "cache")]
    pub fn cache(&self) -> Option<&cordex_cache::InMemoryCache> {
        self.shared.cache.as_deref()
    }

    /// The voice signaling manager, if the bot was built with
    /// [`Bot::voice`](super::Bot::voice).
    #[cfg(feature = "voice")]
    pub fn voice(&self) -> Option<&cordex_voice::VoiceManager> {
        self.shared.voice.as_deref()
    }

    /// Wait for the next event matching `pred`, or `None` after `timeout`.
    ///
    /// Only events the bot parses arrive here: those of registered handlers,
    /// the cache, and [`Bot::parse_events`](super::Bot::parse_events). Their
    /// intents must be on too ([`Bot::intents`](super::Bot::intents)),
    /// otherwise the wait simply times out.
    pub async fn wait_for<F>(&self, timeout: Duration, pred: F) -> Option<Event>
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        self.shared.standby.wait_for(timeout, pred).await
    }

    /// Wait for a button press or select on `message_id`, answerable like in
    /// an `on_component` handler (`ctx.update(..)`, `ctx.reply(..)`). The
    /// interaction is not passed to `on_component` handlers as well.
    /// `INTERACTION_CREATE` is parsed when any command or component handler
    /// is registered (or via [`Bot::parse_events`](super::Bot::parse_events)).
    pub async fn wait_for_button(
        &self,
        message_id: Id<MessageMarker>,
        timeout: Duration,
    ) -> Option<ComponentContext> {
        let event = self.wait_for(timeout, component_on(message_id)).await?;
        self.component_context(event)
    }

    /// All button presses and selects on `message_id` until `timeout`
    /// (counted from now) or until the returned value is dropped, e.g. for
    /// polls or paging. Each click is answerable like in `on_component`
    /// and is not passed to `on_component` handlers. Buffers up to
    /// [`STANDBY_STREAM_CAPACITY`](cordex_gateway::STANDBY_STREAM_CAPACITY)
    /// clicks.
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use cordex::prelude::*;
    /// # async fn poll(ctx: CommandContext, message: Message) -> cordex::Result<()> {
    /// let mut clicks = ctx.button_clicks(message.id, Duration::from_secs(60));
    /// while let Some(click) = clicks.next().await {
    ///     click.reply_ephemeral("Vote counted").await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn button_clicks(&self, message_id: Id<MessageMarker>, timeout: Duration) -> ButtonClicks {
        ButtonClicks {
            ctx: self.clone(),
            events: self
                .shared
                .standby
                .wait_for_stream(component_on(message_id)),
            deadline: Some(tokio::time::Instant::now() + timeout),
            idle: None,
            stopped: false,
        }
    }

    /// Forward an existing message into `destination_channel_id`.
    pub async fn forward_message(
        &self,
        destination_channel_id: Id<ChannelMarker>,
        source_channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> Result<Message> {
        Ok(self
            .http()
            .forward_message(destination_channel_id, source_channel_id, message_id)
            .await?)
    }

    /// Keep Discord's "bot is typing…" indicator active in `channel_id` until
    /// the returned [`Typing`](cordex_http::Typing) guard is dropped.
    pub fn start_typing(&self, channel_id: Id<ChannelMarker>) -> cordex_http::Typing {
        self.http().start_typing(channel_id)
    }

    /// Update the bot's presence on the shard that delivered this event.
    pub async fn set_presence(
        &self,
        presence: &cordex_model::gateway::presence::UpdatePresence,
    ) -> Result<()> {
        if let Some(shard) = self.shared.manager().shard(self.shard.number()) {
            shard.update_presence(presence).await?;
        }
        Ok(())
    }

    /// `ComponentContext` for a component interaction event.
    fn component_context(&self, event: Event) -> Option<ComponentContext> {
        let Event::InteractionCreate(mut interaction) = event else {
            return None;
        };
        let Some(InteractionData::MessageComponent(data)) = interaction.data.take() else {
            return None;
        };
        let handle = InteractionHandle::new(self.shared.http.clone(), &interaction);
        Some(ComponentContext {
            ctx: self.clone(),
            interaction,
            data,
            handle,
        })
    }

    /// Wait for a message in `channel` matching `pred`. Needs
    /// `MESSAGE_CREATE` parsed (an `on_message` handler or
    /// [`Bot::parse_events`](super::Bot::parse_events)) and the message
    /// intents.
    pub async fn wait_for_message<F>(
        &self,
        channel: Id<ChannelMarker>,
        timeout: Duration,
        pred: F,
    ) -> Option<Message>
    where
        F: Fn(&Message) -> bool + Send + Sync + 'static,
    {
        match self
            .wait_for(timeout, move |event| match event {
                Event::MessageCreate(m) => m.channel_id == channel && pred(m),
                _ => false,
            })
            .await
        {
            Some(Event::MessageCreate(m)) => Some(*m),
            _ => None,
        }
    }

    /// Wait for a reaction on `message_id`. Needs `MESSAGE_REACTION_ADD`
    /// parsed ([`Bot::parse_events`](super::Bot::parse_events)) and the
    /// `GUILD_MESSAGE_REACTIONS` / `DIRECT_MESSAGE_REACTIONS` intents.
    pub async fn wait_for_reaction(
        &self,
        message_id: Id<MessageMarker>,
        timeout: Duration,
    ) -> Option<MessageReactionAdd> {
        match self
            .wait_for(timeout, move |event| match event {
                Event::MessageReactionAdd(r) => r.message_id == message_id,
                _ => false,
            })
            .await
        {
            Some(Event::MessageReactionAdd(r)) => Some(*r),
            _ => None,
        }
    }
}

/// Filter for component interactions on one message.
fn component_on(message_id: Id<MessageMarker>) -> impl Fn(&Event) -> bool + Send + Sync + 'static {
    move |event| match event {
        Event::InteractionCreate(i) => {
            i.kind == InteractionType::MessageComponent
                && i.message.as_ref().is_some_and(|m| m.id == message_id)
        }
        _ => false,
    }
}

/// Clicks on one message, from [`Context::button_clicks`].
pub struct ButtonClicks {
    ctx: Context,
    events: tokio::sync::mpsc::Receiver<Event>,
    deadline: Option<tokio::time::Instant>,
    idle: Option<Duration>,
    stopped: bool,
}

impl fmt::Debug for ButtonClicks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ButtonClicks").finish_non_exhaustive()
    }
}

impl ButtonClicks {
    /// Also stop the stream when no click arrives for `timeout` (sliding idle
    /// timeout, reset on every received click).
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle = Some(timeout);
        self
    }

    /// Stop listening immediately; further calls to [`next`](Self::next)
    /// return `None`.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.events.close();
    }

    /// The next click, or `None` once the timeout has passed or
    /// [`stop`](Self::stop) was called.
    pub async fn next(&mut self) -> Option<ComponentContext> {
        if self.stopped {
            return None;
        }
        loop {
            let idle_deadline = self.idle.map(|d| tokio::time::Instant::now() + d);
            let until = match (self.deadline, idle_deadline) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            let event = match until {
                Some(at) => tokio::time::timeout_at(at, self.events.recv())
                    .await
                    .ok()??,
                None => self.events.recv().await?,
            };
            if let Some(click) = self.ctx.component_context(event) {
                return Some(click);
            }
        }
    }
}

macro_rules! deref_context {
    ($($ty:ty),+) => {$(
        impl Deref for $ty {
            type Target = Context;
            fn deref(&self) -> &Context {
                &self.ctx
            }
        }
    )+};
}

deref_context!(
    CommandContext,
    AutocompleteContext,
    ComponentContext,
    ModalContext,
    MessageContext,
    ReadyContext,
    EventContext
);

/// Methods every interaction context shares (reply, defer, edit, …).
macro_rules! interaction_methods {
    ($($ty:ty),+) => {$(
        impl $ty {
            /// The full interaction (without its `data`, see the typed
            /// accessors).
            pub fn interaction(&self) -> &Interaction {
                &self.interaction
            }

            /// Low-level handle for anything not wrapped here.
            pub fn handle(&self) -> &InteractionHandle {
                &self.handle
            }

            /// User who triggered the interaction.
            pub fn user(&self) -> Option<&User> {
                self.interaction.author()
            }

            /// Guild the interaction came from (`None` in DMs).
            pub fn guild_id(&self) -> Option<Id<GuildMarker>> {
                self.interaction.guild_id
            }

            /// Channel the interaction came from.
            pub fn channel_id(&self) -> Option<Id<ChannelMarker>> {
                self.interaction.channel_id
            }

            /// Permissions the bot holds in the interaction's channel
            /// (precomputed by Discord; no cache or REST call needed).
            pub fn app_permissions(&self) -> cordex_model::Permissions {
                self.interaction.app_permissions
            }

            /// Permissions the invoking member holds in the interaction's
            /// channel (`None` in DMs; precomputed by Discord).
            pub fn member_permissions(&self) -> Option<cordex_model::Permissions> {
                self.interaction
                    .member
                    .as_ref()
                    .and_then(|m| m.permissions)
            }

            /// Whether the bot holds `permission` (or `ADMINISTRATOR`) in the
            /// interaction's channel.
            pub fn bot_can(&self, permission: cordex_model::Permissions) -> bool {
                let p = self.app_permissions();
                p.contains(cordex_model::Permissions::ADMINISTRATOR) || p.contains(permission)
            }

            /// Whether the invoking member holds `permission` (or
            /// `ADMINISTRATOR`) in the interaction's channel.
            pub fn user_can(&self, permission: cordex_model::Permissions) -> bool {
                self.member_permissions().is_some_and(|p| {
                    p.contains(cordex_model::Permissions::ADMINISTRATOR) || p.contains(permission)
                })
            }

            /// Answer with a message. After a defer (also the automatic
            /// one) this edits the deferred response instead.
            pub async fn reply(&self, reply: impl Into<Reply>) -> Result<()> {
                Ok(self.handle.respond(reply).await?)
            }

            /// Answer with a message only the user sees.
            ///
            /// If the interaction was already deferred publicly (for
            /// example by auto-defer), Discord no longer allows an
            /// ephemeral answer; the text is then sent as an ephemeral
            /// follow-up and the "thinking" response is removed.
            pub async fn reply_ephemeral(&self, reply: impl Into<Reply>) -> Result<()> {
                let reply = reply.into().ephemeral();
                if self.handle.state().await == ResponseState::Deferred {
                    self.handle.followup(reply).await?;
                    let _ = self.handle.delete_original().await;
                    return Ok(());
                }
                Ok(self.handle.respond(reply).await?)
            }

            /// Edit the response (after `reply` or a defer).
            pub async fn edit(&self, reply: impl Into<Reply>) -> Result<Message> {
                Ok(self.handle.edit(reply).await?)
            }

            /// Send another message (after `reply` or a defer).
            pub async fn followup(&self, reply: impl Into<Reply>) -> Result<Message> {
                Ok(self.handle.followup(reply).await?)
            }
        }
    )+};
}

interaction_methods!(CommandContext, ComponentContext, ModalContext);

macro_rules! defer_methods {
    ($($ty:ty),+) => {$(
        impl $ty {
            /// Show "thinking…"; the answer follows with `reply`.
            pub async fn defer(&self) -> Result<()> {
                Ok(self.handle.defer().await?)
            }

            /// Like [`defer`](Self::defer), but the answer is ephemeral.
            pub async fn defer_ephemeral(&self) -> Result<()> {
                Ok(self.handle.defer_ephemeral().await?)
            }
        }
    )+};
}

defer_methods!(CommandContext, ModalContext);

macro_rules! modal_method {
    ($($ty:ty),+) => {$(
        impl $ty {
            /// Open a [`Modal`]; its submit goes to
            /// [`Bot::on_modal`](super::Bot::on_modal) with the same
            /// `custom_id`. Must be the first answer (not after a defer).
            pub async fn modal(&self, modal: Modal) -> Result<()> {
                Ok(self.handle.show_modal(modal).await?)
            }
        }
    )+};
}

modal_method!(CommandContext, ComponentContext);

/// A slash, user or message command.
#[derive(Clone)]
pub struct CommandContext {
    pub(super) ctx: Context,
    pub(super) interaction: Box<Interaction>,
    pub(super) data: CommandData,
    pub(super) handle: InteractionHandle,
}

impl fmt::Debug for CommandContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandContext")
            .field("command", &self.data.name)
            .finish_non_exhaustive()
    }
}

impl CommandContext {
    /// Command name.
    pub fn name(&self) -> &str {
        &self.data.name
    }

    /// All options, for the rarer types and optional ones
    /// (`ctx.options().string_opt("x")?`).
    pub fn options(&self) -> CommandOptions<'_> {
        CommandOptions::new(&self.data)
    }

    /// Path of the invoked subcommand (`["group", "sub"]`), empty without.
    pub fn subcommand(&self) -> Vec<&str> {
        self.options().subcommand()
    }

    /// Required string option.
    pub fn string_option(&self, name: &str) -> Result<&str> {
        Ok(self.options().string(name)?)
    }

    /// Required integer option.
    pub fn integer_option(&self, name: &str) -> Result<i64> {
        Ok(self.options().integer(name)?)
    }

    /// Required number option.
    pub fn number_option(&self, name: &str) -> Result<f64> {
        Ok(self.options().number(name)?)
    }

    /// Optional boolean option (`false` when not given).
    pub fn boolean_option(&self, name: &str) -> Result<bool> {
        Ok(self.options().boolean_opt(name)?.unwrap_or(false))
    }

    /// Required user option.
    pub fn user_option(&self, name: &str) -> Result<&User> {
        Ok(self.options().user(name)?.user)
    }

    /// Required user option with the member data (in guilds).
    pub fn member_option(&self, name: &str) -> Result<ResolvedUser<'_>> {
        Ok(self.options().user(name)?)
    }

    /// Required role option.
    pub fn role_option(&self, name: &str) -> Result<&Role> {
        Ok(self.options().role(name)?)
    }

    /// Required channel option.
    pub fn channel_option(&self, name: &str) -> Result<&Channel> {
        Ok(self.options().channel(name)?)
    }

    /// Required attachment option.
    pub fn attachment_option(&self, name: &str) -> Result<&Attachment> {
        Ok(self.options().attachment(name)?)
    }

    /// Optional string option.
    pub fn string_opt(&self, name: &str) -> Result<Option<&str>> {
        Ok(self.options().string_opt(name)?)
    }

    /// Optional integer option.
    pub fn integer_opt(&self, name: &str) -> Result<Option<i64>> {
        Ok(self.options().integer_opt(name)?)
    }

    /// Optional number option.
    pub fn number_opt(&self, name: &str) -> Result<Option<f64>> {
        Ok(self.options().number_opt(name)?)
    }

    /// Optional boolean option (`None` when not provided).
    pub fn boolean_opt(&self, name: &str) -> Result<Option<bool>> {
        Ok(self.options().boolean_opt(name)?)
    }

    /// Optional user option.
    pub fn user_opt(&self, name: &str) -> Result<Option<&User>> {
        Ok(self.options().user_opt(name)?.map(|u| u.user))
    }

    /// Optional user option with member data (in guilds).
    pub fn member_opt(&self, name: &str) -> Result<Option<ResolvedUser<'_>>> {
        Ok(self.options().user_opt(name)?)
    }

    /// Optional role option.
    pub fn role_opt(&self, name: &str) -> Result<Option<&Role>> {
        Ok(self.options().role_opt(name)?)
    }

    /// Optional channel option.
    pub fn channel_opt(&self, name: &str) -> Result<Option<&Channel>> {
        Ok(self.options().channel_opt(name)?)
    }

    /// Optional attachment option.
    pub fn attachment_opt(&self, name: &str) -> Result<Option<&Attachment>> {
        Ok(self.options().attachment_opt(name)?)
    }

    /// Target of a user context-menu command.
    pub fn target_user(&self) -> Result<&User> {
        Ok(self.options().target_user()?.user)
    }

    /// Target of a message context-menu command.
    pub fn target_message(&self) -> Result<&Message> {
        Ok(self.options().target_message()?)
    }

    /// Show `pages` with `◀` / `▶` pagination buttons until `timeout` expires
    /// after the last click (idle timeout, then disables the buttons). Only
    /// the invoking user can change pages; other users receive a short
    /// ephemeral notice.
    pub async fn paginate(
        &self,
        pages: impl IntoIterator<Item = Reply>,
        timeout: Duration,
    ) -> Result<()> {
        let pages: Vec<Reply> = pages.into_iter().collect();
        if pages.is_empty() {
            return Ok(());
        }
        if pages.len() == 1 {
            return self.reply(pages[0].clone()).await;
        }
        let author = self.user().map(|u| u.id);
        let mut index = 0usize;
        let total = pages.len();

        let page_reply = |idx: usize, disabled: bool| {
            let mut prev = cordex_model::component::Button::secondary("cordex_page:prev", "◀");
            if disabled || idx == 0 {
                prev = prev.disabled();
            }
            let indicator = cordex_model::component::Button::secondary(
                "cordex_page:indicator",
                format!("{} / {total}", idx + 1),
            )
            .disabled();
            let mut next = cordex_model::component::Button::secondary("cordex_page:next", "▶");
            if disabled || idx + 1 >= total {
                next = next.disabled();
            }
            let row = cordex_model::component::ActionRow::new([prev, indicator, next]);
            pages[idx].clone().components(vec![row.into()])
        };

        self.defer().await?;
        let message = self.edit(page_reply(index, false)).await?;
        let mut clicks = ButtonClicks {
            ctx: self.ctx.clone(),
            events: self
                .shared
                .standby
                .wait_for_stream(component_on(message.id)),
            deadline: None,
            idle: Some(timeout),
            stopped: false,
        };
        while let Some(click) = clicks.next().await {
            if author.is_some() && click.user().map(|u| u.id) != author {
                let _ = click
                    .reply_ephemeral("Only the command author can change pages.")
                    .await;
                continue;
            }
            match click.custom_id() {
                "cordex_page:prev" if index > 0 => index -= 1,
                "cordex_page:next" if index + 1 < total => index += 1,
                _ => continue,
            }
            let _ = click.update(page_reply(index, false)).await;
        }
        let _ = self.edit(page_reply(index, true)).await;
        Ok(())
    }
}

/// Autocomplete request for a command option.
pub struct AutocompleteContext {
    pub(super) ctx: Context,
    pub(super) interaction: Box<Interaction>,
    pub(super) data: CommandData,
    pub(super) handle: InteractionHandle,
}

impl fmt::Debug for AutocompleteContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutocompleteContext")
            .field("command", &self.data.name)
            .finish_non_exhaustive()
    }
}

impl AutocompleteContext {
    /// Command name.
    pub fn name(&self) -> &str {
        &self.data.name
    }

    /// The interaction.
    pub fn interaction(&self) -> &Interaction {
        &self.interaction
    }

    /// All options typed so far.
    pub fn options(&self) -> CommandOptions<'_> {
        CommandOptions::new(&self.data)
    }

    /// Name of the option the user is typing in.
    pub fn focused_name(&self) -> Option<&str> {
        self.options().focused().map(|o| &*o.name)
    }

    /// What the user typed so far (text; numbers as written).
    pub fn focused_value(&self) -> String {
        match self.options().focused().and_then(|o| o.value.as_ref()) {
            Some(CommandOptionValue::String(s)) => s.to_string(),
            Some(CommandOptionValue::Integer(i)) => i.to_string(),
            Some(CommandOptionValue::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    }

    /// Send up to 25 suggestions (more are cut off).
    ///
    /// The choices are collected before anything is awaited, so a lazy
    /// iterator with closures works here.
    pub fn suggest(
        &self,
        choices: impl IntoIterator<Item = Choice>,
    ) -> impl Future<Output = Result<()>> + Send + '_ {
        let choices: Vec<Choice> = choices.into_iter().take(25).collect();
        async move { Ok(self.handle.autocomplete(choices).await?) }
    }
}

/// A button press or select menu choice.
pub struct ComponentContext {
    pub(super) ctx: Context,
    pub(super) interaction: Box<Interaction>,
    pub(super) data: MessageComponentData,
    pub(super) handle: InteractionHandle,
}

impl fmt::Debug for ComponentContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentContext")
            .field("custom_id", &self.data.custom_id)
            .finish_non_exhaustive()
    }
}

impl ComponentContext {
    /// Full `custom_id` of the component.
    pub fn custom_id(&self) -> &str {
        &self.data.custom_id
    }

    /// Text after the first `:` of the `custom_id` (`"vote:42"` → `"42"`),
    /// empty without one.
    pub fn custom_id_arg(&self) -> &str {
        self.data
            .custom_id
            .split_once(':')
            .map_or("", |(_, arg)| arg)
    }

    /// Selected values of a select menu (empty for buttons).
    pub fn values(&self) -> &[Box<str>] {
        &self.data.values
    }

    /// Message the component is attached to.
    pub fn message(&self) -> Option<&Message> {
        self.interaction.message.as_deref()
    }

    /// Replace the message the component is attached to.
    pub async fn update(&self, reply: impl Into<Reply>) -> Result<()> {
        Ok(self.handle.update_message(reply).await?)
    }

    /// Acknowledge without changing anything yet; edit later.
    pub async fn defer_update(&self) -> Result<()> {
        Ok(self.handle.defer_update().await?)
    }
}

/// A submitted modal.
pub struct ModalContext {
    pub(super) ctx: Context,
    pub(super) interaction: Box<Interaction>,
    pub(super) data: ModalSubmitData,
    pub(super) handle: InteractionHandle,
}

impl fmt::Debug for ModalContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModalContext")
            .field("custom_id", &self.data.custom_id)
            .finish_non_exhaustive()
    }
}

impl ModalContext {
    /// Full `custom_id` of the modal.
    pub fn custom_id(&self) -> &str {
        &self.data.custom_id
    }

    /// Text after the first `:` of the `custom_id`, empty without one.
    pub fn custom_id_arg(&self) -> &str {
        self.data
            .custom_id
            .split_once(':')
            .map_or("", |(_, arg)| arg)
    }

    /// Value of the text input or radio group with this `custom_id`
    /// (searches nested rows and labels). `None` for an empty radio group.
    pub fn value(&self, custom_id: &str) -> Option<&str> {
        find_component(&self.data.components, custom_id)
            .and_then(|c| c.value.as_ref())
            .and_then(SubmittedValue::as_str)
    }

    /// Whether the checkbox with this `custom_id` is checked (`false` if
    /// it is missing).
    pub fn checked(&self, custom_id: &str) -> bool {
        find_component(&self.data.components, custom_id)
            .and_then(|c| c.value.as_ref())
            .and_then(SubmittedValue::as_bool)
            .unwrap_or(false)
    }

    /// Selected values of the select or checkbox group with this
    /// `custom_id`.
    pub fn values(&self, custom_id: &str) -> &[Box<str>] {
        find_component(&self.data.components, custom_id).map_or(&[], |c| &c.values)
    }
}

fn find_component<'a>(
    list: &'a [SubmittedComponent],
    custom_id: &str,
) -> Option<&'a SubmittedComponent> {
    list.iter().find_map(|c| {
        if c.custom_id.as_deref() == Some(custom_id) {
            return Some(c);
        }
        find_component(&c.components, custom_id).or_else(|| {
            c.component
                .as_deref()
                .and_then(|inner| find_component(std::slice::from_ref(inner), custom_id))
        })
    })
}

/// A new message (`MESSAGE_CREATE`). Messages of the bot itself are not
/// delivered.
pub struct MessageContext {
    pub(super) ctx: Context,
    pub(super) message: Arc<Message>,
}

impl fmt::Debug for MessageContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageContext")
            .field("id", &self.message.id)
            .finish_non_exhaustive()
    }
}

impl MessageContext {
    /// The message.
    pub fn message(&self) -> &Message {
        &self.message
    }

    /// Text of the message. Empty in guilds unless the `MESSAGE_CONTENT`
    /// intent is on (see [`Bot::message_content`](super::Bot::message_content))
    /// or the bot is mentioned.
    pub fn content(&self) -> &str {
        &self.message.content
    }

    /// Author of the message.
    pub fn author(&self) -> &User {
        &self.message.author
    }

    /// Channel of the message.
    pub fn channel_id(&self) -> Id<ChannelMarker> {
        self.message.channel_id
    }

    /// Guild of the message (`None` in DMs).
    pub fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.message.guild_id
    }

    /// Reply to the message (shows the reference).
    pub async fn reply(&self, content: impl Into<String>) -> Result<Message> {
        Ok(self
            .ctx
            .shared
            .http
            .create_message(self.message.channel_id)
            .content(content)?
            .reply(self.message.id)
            .send()
            .await?)
    }

    /// Send a message to the same channel.
    pub async fn send(&self, content: impl Into<String>) -> Result<Message> {
        Ok(self
            .ctx
            .shared
            .http
            .create_message(self.message.channel_id)
            .content(content)?
            .send()
            .await?)
    }

    /// Add a reaction (`"👍"` or `"name:id"`) to this message.
    pub async fn react(&self, emoji: impl Into<String>) -> Result<()> {
        Ok(self
            .ctx
            .shared
            .http
            .create_reaction(self.message.channel_id, self.message.id, emoji)
            .await?)
    }

    /// Delete this message.
    pub async fn delete(&self) -> Result<()> {
        Ok(self
            .ctx
            .shared
            .http
            .delete_message(self.message.channel_id, self.message.id)
            .await?)
    }
}

/// A shard is connected (`READY`).
pub struct ReadyContext {
    pub(super) ctx: Context,
    pub(super) ready: Arc<Ready>,
}

impl fmt::Debug for ReadyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadyContext")
            .field("shard", &self.ctx.shard)
            .finish_non_exhaustive()
    }
}

impl ReadyContext {
    /// The full `READY` payload.
    pub fn ready(&self) -> &Ready {
        &self.ready
    }

    /// The bot user.
    pub fn bot_user(&self) -> &User {
        &self.ready.user
    }

    /// Number of guilds on this shard.
    pub fn guild_count(&self) -> usize {
        self.ready.guilds.len()
    }
}

/// Any gateway event, for [`Bot::on_event`](super::Bot::on_event).
pub struct EventContext {
    pub(super) ctx: Context,
    pub(super) event: Arc<Event>,
    #[cfg(feature = "cache")]
    pub(super) previous: Arc<cordex_cache::PreviousState>,
}

impl fmt::Debug for EventContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventContext")
            .field("event", &self.event.name())
            .finish_non_exhaustive()
    }
}

impl EventContext {
    /// The event.
    pub fn event(&self) -> &Event {
        &self.event
    }

    /// Cached state replaced or removed by this event, when the cache is
    /// enabled and held an entry before the update.
    #[cfg(feature = "cache")]
    pub fn previous(&self) -> &cordex_cache::PreviousState {
        &self.previous
    }
}
