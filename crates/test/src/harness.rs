//! [`TestBot`] offline harness, event/interaction builders, and `.jsonl` replay.

use std::num::NonZeroU64;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use cordex::bot::{Bot, Context, OfflineBot};
use cordex_gateway::recorder::RecordedFrame;
use cordex_gateway::{ShardId, ShardManager};
use cordex_model::channel::Channel;
use cordex_model::gateway::Intents;
use cordex_model::gateway::event::Event;
use cordex_model::guild::{Guild, Member};
use cordex_model::id::{ApplicationId, ChannelId, GuildId, MessageId, UserId};
use cordex_model::user::User;
use serde_json::{Value, json, value::RawValue};

use crate::error::TestError;
use crate::recorder::{MockHttp, RecordedRequest};
use crate::result::{EventResult, InteractionResult};

/// Offline test harness wrapping a normal [`Bot`] without any network I/O.
pub struct TestBot {
    bot: OfflineBot,
    http: MockHttp,
    app_id: ApplicationId,
    default_guild_id: GuildId,
    default_channel_id: ChannelId,
    default_user_id: UserId,
    next_id: AtomicU64,
}

impl TestBot {
    /// Wrap a [`Bot`] in an offline harness with an in-memory mock HTTP recorder.
    pub async fn new(bot: Bot) -> Result<Self, TestError> {
        let http = MockHttp::new();
        let client = http.client()?;
        let manager = ShardManager::builder("mock.token.secret", Intents::empty())
            .http(client.clone())
            .shards(0..1, 1)
            .build()
            .await?;
        http.clear();
        let offline = bot.into_offline(client, manager);
        Ok(Self {
            bot: offline,
            http,
            app_id: ApplicationId::new(100_000_000_000_000_001)
                .unwrap_or_else(|| ApplicationId::from(NonZeroU64::MIN)),
            default_guild_id: GuildId::new(200_000_000_000_000_001)
                .unwrap_or_else(|| GuildId::from(NonZeroU64::MIN)),
            default_channel_id: ChannelId::new(300_000_000_000_000_001)
                .unwrap_or_else(|| ChannelId::from(NonZeroU64::MIN)),
            default_user_id: UserId::new(400_000_000_000_000_001)
                .unwrap_or_else(|| UserId::from(NonZeroU64::MIN)),
            next_id: AtomicU64::new(500_000_000_000_000_001),
        })
    }

    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Access the underlying [`MockHttp`] recorder to inspect requests or inject errors.
    pub fn mock_http(&self) -> &MockHttp {
        &self.http
    }

    /// Snapshot of all outgoing HTTP requests recorded so far.
    pub fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.http.requests()
    }

    /// Build a [`Context`] bound to this offline bot.
    pub fn context(&self) -> Context {
        self.bot.context()
    }

    /// Pre-seed a [`Guild`] into the bot's cache (if caching is enabled).
    pub fn seed_guild(&self, guild: Guild) {
        if let Some(cache) = self.bot.cache() {
            cache.insert_guild(guild);
        }
    }

    /// Pre-seed a [`Channel`] into the bot's cache (if caching is enabled).
    pub fn seed_channel(&self, channel: Channel) {
        if let Some(cache) = self.bot.cache() {
            cache.insert_channel(channel);
        }
    }

    /// Pre-seed a [`User`] into the bot's cache (if caching is enabled).
    pub fn seed_user(&self, user: User) {
        if let Some(cache) = self.bot.cache() {
            cache.insert_user(user);
        }
    }

    /// Pre-seed a [`Member`] into the bot's cache (if caching is enabled).
    pub fn seed_member(&self, guild_id: GuildId, member: &Member) {
        if let Some(cache) = self.bot.cache() {
            cache.insert_guild_member(guild_id, member);
        }
    }

    /// Dispatch a raw [`Event`] on shard 0 without waiting for spawned handlers.
    pub fn dispatch_now(&self, event: Event) {
        self.bot.dispatch(ShardId::one(), event);
    }

    /// Dispatch a raw [`Event`] and yield until spawned handler tasks settle.
    pub async fn dispatch(&self, event: Event) -> EventResult {
        let start = self.http.len();
        self.bot.dispatch(ShardId::one(), event);
        settle().await;
        EventResult::new(self.http.requests_since(start))
    }

    /// Start building a slash-command interaction (`INTERACTION_CREATE`, type 2).
    pub fn slash(&self, name: impl Into<String>) -> SlashBuilder<'_> {
        SlashBuilder {
            harness: self,
            name: name.into(),
            options: Vec::new(),
            resolved_users: serde_json::Map::new(),
            user_id: self.default_user_id,
            guild_id: Some(self.default_guild_id),
            channel_id: self.default_channel_id,
        }
    }

    /// Start building a button-click interaction (`INTERACTION_CREATE`, type 3).
    pub fn click_button(
        &self,
        message_id: MessageId,
        custom_id: impl Into<String>,
    ) -> ComponentBuilder<'_> {
        ComponentBuilder {
            harness: self,
            message_id,
            custom_id: custom_id.into(),
            user_id: self.default_user_id,
            guild_id: Some(self.default_guild_id),
            channel_id: self.default_channel_id,
        }
    }

    /// Start building a modal-submit interaction (`INTERACTION_CREATE`, type 5).
    pub fn submit_modal<K, V>(
        &self,
        custom_id: impl Into<String>,
        fields: impl IntoIterator<Item = (K, V)>,
    ) -> ModalBuilder<'_>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let fields = fields
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        ModalBuilder {
            harness: self,
            custom_id: custom_id.into(),
            fields,
            user_id: self.default_user_id,
            guild_id: Some(self.default_guild_id),
            channel_id: self.default_channel_id,
        }
    }

    /// Start building an autocomplete interaction (`INTERACTION_CREATE`, type 4).
    pub fn trigger_autocomplete(
        &self,
        command: impl Into<String>,
        option: impl Into<String>,
        value: impl Into<String>,
    ) -> AutocompleteBuilder<'_> {
        AutocompleteBuilder {
            harness: self,
            command: command.into(),
            option: option.into(),
            value: value.into(),
            user_id: self.default_user_id,
            guild_id: Some(self.default_guild_id),
            channel_id: self.default_channel_id,
        }
    }

    /// Start building a `MESSAGE_CREATE` event.
    pub fn send_message(
        &self,
        channel_id: ChannelId,
        content: impl Into<String>,
    ) -> MessageBuilder<'_> {
        MessageBuilder {
            harness: self,
            channel_id,
            content: content.into(),
            user_id: self.default_user_id,
            guild_id: Some(self.default_guild_id),
        }
    }

    /// Replay a `.jsonl` gateway capture file through the bot's cache, standby, and handlers.
    ///
    /// Returns the number of frames replayed.
    pub async fn replay_jsonl(&self, path: impl AsRef<Path>) -> Result<usize, TestError> {
        let content = std::fs::read_to_string(path)?;
        self.replay_jsonl_str(&content).await
    }

    /// Replay `.jsonl` gateway capture content from a string.
    ///
    /// Returns the number of frames replayed.
    pub async fn replay_jsonl_str(&self, content: &str) -> Result<usize, TestError> {
        let mut count = 0usize;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let frame = RecordedFrame::from_json_line(trimmed)?;
            let raw = RawValue::from_string(serde_json::to_string(&frame.payload)?)?;
            let event = Event::from_dispatch(&frame.event_name, Some(&raw))?;
            self.bot.dispatch(ShardId::one(), event);
            settle().await;
            count += 1;
        }
        Ok(count)
    }

    async fn dispatch_interaction_json(&self, payload: &Value) -> InteractionResult {
        let start = self.http.len();
        if let Ok(raw_str) = serde_json::to_string(payload)
            && let Ok(raw) = RawValue::from_string(raw_str)
            && let Ok(event) = Event::from_dispatch("INTERACTION_CREATE", Some(&raw))
        {
            self.bot.dispatch(ShardId::one(), event);
            settle().await;
        }
        InteractionResult::new(self.http.requests_since(start))
    }
}

/// Yield to the Tokio runtime so spawned handler tasks complete.
pub async fn settle() {
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
}

/// Builder for a slash-command interaction.
pub struct SlashBuilder<'a> {
    harness: &'a TestBot,
    name: String,
    options: Vec<Value>,
    resolved_users: serde_json::Map<String, Value>,
    user_id: UserId,
    guild_id: Option<GuildId>,
    channel_id: ChannelId,
}

impl SlashBuilder<'_> {
    /// Add a string option (`type = 3`).
    #[must_use]
    pub fn string_opt(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.push(json!({
            "name": name.into(),
            "type": 3,
            "value": value.into(),
        }));
        self
    }

    /// Add an integer option (`type = 4`).
    #[must_use]
    pub fn int_opt(mut self, name: impl Into<String>, value: i64) -> Self {
        self.options.push(json!({
            "name": name.into(),
            "type": 4,
            "value": value,
        }));
        self
    }

    /// Add a boolean option (`type = 5`).
    #[must_use]
    pub fn bool_opt(mut self, name: impl Into<String>, value: bool) -> Self {
        self.options.push(json!({
            "name": name.into(),
            "type": 5,
            "value": value,
        }));
        self
    }

    /// Add a user option (`type = 6`).
    #[must_use]
    pub fn user_opt(mut self, name: impl Into<String>, user_id: UserId) -> Self {
        let id_str = user_id.get().to_string();
        self.options.push(json!({
            "name": name.into(),
            "type": 6,
            "value": id_str,
        }));
        self.resolved_users.insert(
            id_str.clone(),
            json!({
                "id": id_str,
                "username": format!("user_{}", user_id.get()),
                "discriminator": "0",
                "bot": false
            }),
        );
        self
    }

    /// Set the invoking user ID.
    #[must_use]
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the guild ID (or `None` for DM).
    #[must_use]
    pub fn in_guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Set the channel ID.
    #[must_use]
    pub fn in_channel(mut self, channel_id: ChannelId) -> Self {
        self.channel_id = channel_id;
        self
    }

    /// Dispatch the slash command and wait for the handler to settle.
    pub async fn send(self) -> InteractionResult {
        let id = self.harness.alloc_id();
        let cmd_id = self.harness.alloc_id();
        let mut data = json!({
            "id": cmd_id.to_string(),
            "name": self.name,
            "type": 1,
            "options": self.options,
        });
        if !self.resolved_users.is_empty()
            && let Some(obj) = data.as_object_mut()
        {
            obj.insert(
                "resolved".into(),
                json!({ "users": Value::Object(self.resolved_users) }),
            );
        }
        let payload = base_interaction(
            id,
            self.harness.app_id,
            2,
            self.user_id,
            self.guild_id,
            self.channel_id,
            data,
            None,
        );
        self.harness.dispatch_interaction_json(&payload).await
    }
}

/// Builder for a button-click interaction.
pub struct ComponentBuilder<'a> {
    harness: &'a TestBot,
    message_id: MessageId,
    custom_id: String,
    user_id: UserId,
    guild_id: Option<GuildId>,
    channel_id: ChannelId,
}

impl ComponentBuilder<'_> {
    /// Set the clicking user ID.
    #[must_use]
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the guild ID.
    #[must_use]
    pub fn in_guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Set the channel ID.
    #[must_use]
    pub fn in_channel(mut self, channel_id: ChannelId) -> Self {
        self.channel_id = channel_id;
        self
    }

    /// Dispatch the button click and wait for handlers to settle.
    pub async fn send(self) -> InteractionResult {
        let id = self.harness.alloc_id();
        let message = json!({
            "id": self.message_id.get().to_string(),
            "channel_id": self.channel_id.get().to_string(),
            "author": {
                "id": self.harness.app_id.get().to_string(),
                "username": "test-bot",
                "discriminator": "0",
                "bot": true
            },
            "content": "",
            "timestamp": "2026-01-01T00:00:00.000000+00:00",
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "type": 0
        });
        let payload = base_interaction(
            id,
            self.harness.app_id,
            3,
            self.user_id,
            self.guild_id,
            self.channel_id,
            json!({
                "custom_id": self.custom_id,
                "component_type": 2,
            }),
            Some(message),
        );
        self.harness.dispatch_interaction_json(&payload).await
    }
}

/// Builder for a modal-submit interaction.
pub struct ModalBuilder<'a> {
    harness: &'a TestBot,
    custom_id: String,
    fields: Vec<(String, String)>,
    user_id: UserId,
    guild_id: Option<GuildId>,
    channel_id: ChannelId,
}

impl ModalBuilder<'_> {
    /// Set the submitting user ID.
    #[must_use]
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the guild ID.
    #[must_use]
    pub fn in_guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Set the channel ID.
    #[must_use]
    pub fn in_channel(mut self, channel_id: ChannelId) -> Self {
        self.channel_id = channel_id;
        self
    }

    /// Dispatch the modal submission and wait for handlers to settle.
    pub async fn send(self) -> InteractionResult {
        let id = self.harness.alloc_id();
        let rows: Vec<Value> = self
            .fields
            .into_iter()
            .map(|(k, v)| {
                json!({
                    "type": 1,
                    "components": [{
                        "type": 4,
                        "custom_id": k,
                        "value": v,
                    }]
                })
            })
            .collect();
        let payload = base_interaction(
            id,
            self.harness.app_id,
            5,
            self.user_id,
            self.guild_id,
            self.channel_id,
            json!({
                "custom_id": self.custom_id,
                "components": rows,
            }),
            None,
        );
        self.harness.dispatch_interaction_json(&payload).await
    }
}

/// Builder for an autocomplete interaction.
pub struct AutocompleteBuilder<'a> {
    harness: &'a TestBot,
    command: String,
    option: String,
    value: String,
    user_id: UserId,
    guild_id: Option<GuildId>,
    channel_id: ChannelId,
}

impl AutocompleteBuilder<'_> {
    /// Set the invoking user ID.
    #[must_use]
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the guild ID.
    #[must_use]
    pub fn in_guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Set the channel ID.
    #[must_use]
    pub fn in_channel(mut self, channel_id: ChannelId) -> Self {
        self.channel_id = channel_id;
        self
    }

    /// Dispatch the autocomplete interaction and wait for handlers to settle.
    pub async fn send(self) -> InteractionResult {
        let id = self.harness.alloc_id();
        let cmd_id = self.harness.alloc_id();
        let payload = base_interaction(
            id,
            self.harness.app_id,
            4,
            self.user_id,
            self.guild_id,
            self.channel_id,
            json!({
                "id": cmd_id.to_string(),
                "name": self.command,
                "type": 1,
                "options": [{
                    "name": self.option,
                    "type": 3,
                    "value": self.value,
                    "focused": true,
                }],
            }),
            None,
        );
        self.harness.dispatch_interaction_json(&payload).await
    }
}

/// Builder for a `MESSAGE_CREATE` gateway event.
pub struct MessageBuilder<'a> {
    harness: &'a TestBot,
    channel_id: ChannelId,
    content: String,
    user_id: UserId,
    guild_id: Option<GuildId>,
}

impl MessageBuilder<'_> {
    /// Set the message author ID.
    #[must_use]
    pub fn as_user(mut self, user_id: UserId) -> Self {
        self.user_id = user_id;
        self
    }

    /// Set the guild ID.
    #[must_use]
    pub fn in_guild(mut self, guild_id: GuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    /// Dispatch the `MESSAGE_CREATE` event and wait for handlers to settle.
    pub async fn send(self) -> EventResult {
        let msg_id = self.harness.alloc_id();
        let mut payload = json!({
            "id": msg_id.to_string(),
            "channel_id": self.channel_id.get().to_string(),
            "author": {
                "id": self.user_id.get().to_string(),
                "username": "test-user",
                "discriminator": "0",
                "bot": false
            },
            "content": self.content,
            "timestamp": "2026-01-01T00:00:00.000000+00:00",
            "tts": false,
            "mention_everyone": false,
            "mentions": [],
            "mention_roles": [],
            "attachments": [],
            "embeds": [],
            "pinned": false,
            "type": 0
        });
        if let Some(guild_id) = self.guild_id
            && let Some(obj) = payload.as_object_mut()
        {
            obj.insert("guild_id".into(), Value::String(guild_id.get().to_string()));
        }
        let start = self.harness.http.len();
        if let Ok(raw_str) = serde_json::to_string(&payload)
            && let Ok(raw) = RawValue::from_string(raw_str)
            && let Ok(event) = Event::from_dispatch("MESSAGE_CREATE", Some(&raw))
        {
            self.harness.bot.dispatch(ShardId::one(), event);
            settle().await;
        }
        EventResult::new(self.harness.http.requests_since(start))
    }
}

fn base_interaction(
    id: u64,
    app_id: ApplicationId,
    kind: u8,
    user_id: UserId,
    guild_id: Option<GuildId>,
    channel_id: ChannelId,
    data: Value,
    message: Option<Value>,
) -> Value {
    let user = json!({
        "id": user_id.get().to_string(),
        "username": "test-user",
        "discriminator": "0",
        "bot": false
    });
    let mut obj = json!({
        "id": id.to_string(),
        "application_id": app_id.get().to_string(),
        "type": kind,
        "token": format!("mock-interaction-token-{id}"),
        "version": 1,
        "channel_id": channel_id.get().to_string(),
    });
    if let Some(map) = obj.as_object_mut() {
        map.insert("data".into(), data);
        if let Some(guild_id) = guild_id {
            map.insert("guild_id".into(), Value::String(guild_id.get().to_string()));
            map.insert(
                "member".into(),
                json!({
                    "user": user,
                    "roles": [],
                    "permissions": "2147483647",
                    "deaf": false,
                    "mute": false,
                    "joined_at": "2026-01-01T00:00:00.000000+00:00",
                }),
            );
        } else {
            map.insert("user".into(), user);
        }
        if let Some(msg) = message {
            map.insert("message".into(), msg);
        }
    }
    obj
}
