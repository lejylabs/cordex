//! Per-guild voice signaling state machine and session coordinator.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cordex_model::gateway::command::UpdateVoiceState;
use cordex_model::gateway::event::{Event, VoiceServerUpdate};
use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, GuildMarker, UserMarker};
use cordex_model::voice::VoiceState;
use parking_lot::Mutex;
use tokio::sync::{broadcast, watch};

use crate::error::VoiceError;
use crate::gateway::VoiceGateway;
use crate::types::{VoiceConnectionInfo, VoiceEvent, VoiceOptions};

/// Default capacity of the [`VoiceManager::subscribe`] broadcast channel.
pub const DEFAULT_EVENT_CAPACITY: usize = 64;

#[derive(Debug, Default)]
struct PartialState {
    channel_id: Option<Id<ChannelMarker>>,
    session_id: Option<Box<str>>,
    self_mute: bool,
    self_deaf: bool,
    token: Option<Box<str>>,
    endpoint: Option<Box<str>>,
    connected: bool,
}

impl PartialState {
    fn to_connection_info(&self, guild_id: Id<GuildMarker>) -> Option<VoiceConnectionInfo> {
        let channel_id = self.channel_id?;
        let session_id = self.session_id.as_ref()?;
        let token = self.token.as_ref()?;
        let endpoint = self.endpoint.as_ref()?;
        Some(VoiceConnectionInfo {
            guild_id,
            channel_id,
            session_id: session_id.clone(),
            token: token.clone(),
            endpoint: endpoint.clone(),
            self_mute: self.self_mute,
            self_deaf: self.self_deaf,
        })
    }

    fn clear(&mut self) {
        self.channel_id = None;
        self.session_id = None;
        self.self_mute = false;
        self.self_deaf = false;
        self.token = None;
        self.endpoint = None;
        self.connected = false;
    }
}

struct GuildEntry {
    partial: PartialState,
    tx: watch::Sender<Option<VoiceConnectionInfo>>,
}

impl GuildEntry {
    fn new() -> Self {
        let (tx, _rx) = watch::channel(None);
        Self {
            partial: PartialState::default(),
            tx,
        }
    }
}

struct Inner {
    user_id: Option<Id<UserMarker>>,
    guilds: HashMap<Id<GuildMarker>, GuildEntry>,
}

/// Coordinates Discord voice signaling (`VOICE_STATE_UPDATE` and
/// `VOICE_SERVER_UPDATE`) across guilds.
pub struct VoiceManager {
    gateway: Arc<dyn VoiceGateway>,
    inner: Mutex<Inner>,
    events: broadcast::Sender<VoiceEvent>,
}

impl VoiceManager {
    /// Create a new voice manager backed by `gateway`.
    ///
    /// The bot's user ID is automatically learned from the first [`Event::Ready`]
    /// passed to [`Self::update_from_event`], or can be set explicitly via
    /// [`Self::with_user_id`] / [`Self::set_user_id`].
    #[must_use]
    pub fn new(gateway: Arc<dyn VoiceGateway>) -> Self {
        Self::with_capacity(gateway, None, DEFAULT_EVENT_CAPACITY)
    }

    /// Create a new voice manager with a known bot user ID.
    #[must_use]
    pub fn with_user_id(gateway: Arc<dyn VoiceGateway>, user_id: Id<UserMarker>) -> Self {
        Self::with_capacity(gateway, Some(user_id), DEFAULT_EVENT_CAPACITY)
    }

    /// Create a new voice manager with a custom broadcast event buffer capacity.
    #[must_use]
    pub fn with_capacity(
        gateway: Arc<dyn VoiceGateway>,
        user_id: Option<Id<UserMarker>>,
        event_capacity: usize,
    ) -> Self {
        let (events, _rx) = broadcast::channel(event_capacity.max(1));
        Self {
            gateway,
            inner: Mutex::new(Inner {
                user_id,
                guilds: HashMap::new(),
            }),
            events,
        }
    }

    /// Set or update the bot's user ID used to filter `VOICE_STATE_UPDATE` events.
    pub fn set_user_id(&self, user_id: Id<UserMarker>) {
        self.inner.lock().user_id = Some(user_id);
    }

    /// Return the currently configured bot user ID, if known.
    #[must_use]
    pub fn user_id(&self) -> Option<Id<UserMarker>> {
        self.inner.lock().user_id
    }

    /// Join a voice channel with default options (`self_mute: false`,
    /// `self_deaf: false`).
    pub async fn join(
        &self,
        guild_id: Id<GuildMarker>,
        channel_id: Id<ChannelMarker>,
    ) -> Result<(), VoiceError> {
        self.join_with(guild_id, channel_id, VoiceOptions::new())
            .await
    }

    /// Join a voice channel with explicit [`VoiceOptions`].
    pub async fn join_with(
        &self,
        guild_id: Id<GuildMarker>,
        channel_id: Id<ChannelMarker>,
        options: VoiceOptions,
    ) -> Result<(), VoiceError> {
        let update = UpdateVoiceState::new(
            guild_id,
            Some(channel_id),
            options.self_mute,
            options.self_deaf,
        );
        self.update_voice_state(update).await
    }

    /// Leave the voice channel in `guild_id`.
    pub async fn leave(&self, guild_id: Id<GuildMarker>) -> Result<(), VoiceError> {
        let update = UpdateVoiceState::new(guild_id, None, false, false);
        self.update_voice_state(update).await
    }

    /// Send a raw opcode 4 [`UpdateVoiceState`] command through the gateway.
    pub async fn update_voice_state(&self, update: UpdateVoiceState) -> Result<(), VoiceError> {
        self.gateway.send_voice_state(update).await
    }

    /// Current complete voice connection info for `guild_id`, if connected.
    #[must_use]
    pub fn connection_info(&self, guild_id: Id<GuildMarker>) -> Option<VoiceConnectionInfo> {
        let inner = self.inner.lock();
        inner
            .guilds
            .get(&guild_id)
            .and_then(|entry| entry.partial.to_connection_info(guild_id))
    }

    /// Subscribe to all [`VoiceEvent`]s across every guild.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<VoiceEvent> {
        self.events.subscribe()
    }

    /// Watch the [`VoiceConnectionInfo`] for a specific guild.
    ///
    /// The returned receiver always holds the latest complete connection info
    /// (`Some(...)`) or `None` when disconnected / not yet correlated.
    #[must_use]
    pub fn watch(&self, guild_id: Id<GuildMarker>) -> watch::Receiver<Option<VoiceConnectionInfo>> {
        let mut inner = self.inner.lock();
        inner
            .guilds
            .entry(guild_id)
            .or_insert_with(GuildEntry::new)
            .tx
            .subscribe()
    }

    /// Wait until a complete [`VoiceConnectionInfo`] is available for `guild_id`,
    /// or `timeout` elapses.
    ///
    /// Returns immediately if `guild_id` already has a complete connection.
    /// Returns [`VoiceError::Disconnected`] if the bot disconnects while waiting,
    /// or [`VoiceError::Timeout`] if `timeout` expires first.
    pub async fn wait_for_connection(
        &self,
        guild_id: Id<GuildMarker>,
        timeout: Duration,
    ) -> Result<VoiceConnectionInfo, VoiceError> {
        let mut rx = self.watch(guild_id);
        if let Some(info) = rx.borrow_and_update().clone() {
            return Ok(info);
        }

        let wait_fut = async {
            rx.changed().await.map_err(|_| VoiceError::Closed)?;
            if let Some(info) = rx.borrow().clone() {
                Ok(info)
            } else {
                // `None` was published while we were waiting -> explicit disconnect.
                Err(VoiceError::Disconnected { guild_id })
            }
        };

        tokio::time::timeout(timeout, wait_fut)
            .await
            .map_err(|_| VoiceError::Timeout { guild_id })?
    }

    /// Feed an incoming gateway [`Event`] into the voice manager.
    ///
    /// Automatically handles:
    /// - [`Event::Ready`]: records the bot's user ID.
    /// - [`Event::VoiceStateUpdate`]: updates the bot's own voice state.
    /// - [`Event::VoiceServerUpdate`]: updates the guild's voice server credentials.
    ///
    /// All other events are ignored at zero cost.
    pub fn update_from_event(&self, event: &Event) {
        match event {
            Event::Ready(ready) => {
                self.set_user_id(ready.user.id);
            }
            Event::VoiceStateUpdate(state) => {
                self.update_voice_state_event(state);
            }
            Event::VoiceServerUpdate(server) => {
                self.update_voice_server_event(server);
            }
            _ => {}
        }
    }

    /// Process a `VOICE_STATE_UPDATE` payload directly.
    ///
    /// Ignored if `state.guild_id` is `None`, or if `state.user_id` does not
    /// match the configured bot user ID.
    pub fn update_voice_state_event(&self, state: &VoiceState) {
        let Some(guild_id) = state.guild_id else {
            return;
        };

        let mut inner = self.inner.lock();
        if let Some(bot_id) = inner.user_id {
            if state.user_id != bot_id {
                return;
            }
        } else {
            tracing::debug!(
                %guild_id,
                user_id = %state.user_id,
                "ignoring VOICE_STATE_UPDATE because bot user_id is not set yet"
            );
            return;
        }

        let entry = inner.guilds.entry(guild_id).or_insert_with(GuildEntry::new);

        let Some(channel_id) = state.channel_id else {
            let was_active = entry.partial.connected
                || entry.partial.channel_id.is_some()
                || entry.partial.token.is_some();
            entry.partial.clear();
            let _ = entry.tx.send(None);
            drop(inner);
            if was_active {
                let _ = self.events.send(VoiceEvent::Disconnected { guild_id });
            }
            return;
        };

        let was_connected = entry.partial.connected;
        entry.partial.channel_id = Some(channel_id);
        entry.partial.session_id = Some(state.session_id.clone());
        entry.partial.self_mute = state.self_mute;
        entry.partial.self_deaf = state.self_deaf;

        if let Some(info) = entry.partial.to_connection_info(guild_id) {
            entry.partial.connected = true;
            let _ = entry.tx.send(Some(info.clone()));
            drop(inner);
            let ev = if was_connected {
                VoiceEvent::StateUpdated(info)
            } else {
                VoiceEvent::Connected(info)
            };
            let _ = self.events.send(ev);
        }
    }

    /// Process a `VOICE_SERVER_UPDATE` payload directly.
    pub fn update_voice_server_event(&self, server: &VoiceServerUpdate) {
        let guild_id = server.guild_id;
        let mut inner = self.inner.lock();
        let entry = inner.guilds.entry(guild_id).or_insert_with(GuildEntry::new);

        entry.partial.token = Some(server.token.clone());
        entry.partial.endpoint.clone_from(&server.endpoint);

        if server.endpoint.is_none() {
            entry.partial.connected = false;
            drop(inner);
            let _ = self
                .events
                .send(VoiceEvent::EndpointReallocating { guild_id });
            return;
        }

        let was_connected = entry.partial.connected;
        if let Some(info) = entry.partial.to_connection_info(guild_id) {
            entry.partial.connected = true;
            let _ = entry.tx.send(Some(info.clone()));
            drop(inner);
            let ev = if was_connected {
                VoiceEvent::ServerUpdated(info)
            } else {
                VoiceEvent::Connected(info)
            };
            let _ = self.events.send(ev);
        }
    }
}

impl std::fmt::Debug for VoiceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.lock();
        f.debug_struct("VoiceManager")
            .field("user_id", &inner.user_id)
            .field("tracked_guilds", &inner.guilds.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gateway::BoxVoiceFuture;

    #[derive(Default)]
    struct MockGateway {
        sent: Mutex<Vec<UpdateVoiceState>>,
    }

    impl MockGateway {
        fn sent(&self) -> Vec<UpdateVoiceState> {
            self.sent.lock().clone()
        }
    }

    impl VoiceGateway for MockGateway {
        fn send_voice_state(&self, update: UpdateVoiceState) -> BoxVoiceFuture<'_> {
            self.sent.lock().push(update);
            Box::pin(async { Ok(()) })
        }
    }

    fn guild(id: u64) -> Id<GuildMarker> {
        Id::new(id).unwrap()
    }

    fn channel(id: u64) -> Id<ChannelMarker> {
        Id::new(id).unwrap()
    }

    fn user(id: u64) -> Id<UserMarker> {
        Id::new(id).unwrap()
    }

    fn make_voice_state(
        guild_id: Id<GuildMarker>,
        channel_id: Option<Id<ChannelMarker>>,
        user_id: Id<UserMarker>,
        session_id: &str,
        self_mute: bool,
        self_deaf: bool,
    ) -> VoiceState {
        let json = serde_json::json!({
            "guild_id": guild_id.to_string(),
            "channel_id": channel_id.map(|c| c.to_string()),
            "user_id": user_id.to_string(),
            "session_id": session_id,
            "deaf": false,
            "mute": false,
            "self_deaf": self_deaf,
            "self_mute": self_mute,
            "suppress": false
        });
        serde_json::from_value(json).unwrap()
    }

    fn make_voice_server(
        guild_id: Id<GuildMarker>,
        token: &str,
        endpoint: Option<&str>,
    ) -> VoiceServerUpdate {
        let json = serde_json::json!({
            "guild_id": guild_id.to_string(),
            "token": token,
            "endpoint": endpoint,
        });
        serde_json::from_value(json).unwrap()
    }

    #[tokio::test]
    async fn join_and_correlate_state_then_server() {
        let gw = Arc::new(MockGateway::default());
        let vm = VoiceManager::with_user_id(Arc::clone(&gw) as Arc<dyn VoiceGateway>, user(100));
        let mut events = vm.subscribe();

        vm.join_with(
            guild(1),
            channel(10),
            VoiceOptions::new().self_deaf(true).self_mute(false),
        )
        .await
        .unwrap();

        let sent = gw.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].guild_id, guild(1));
        assert_eq!(sent[0].channel_id, Some(channel(10)));
        assert!(sent[0].self_deaf);
        assert!(!sent[0].self_mute);

        // State arrives first -> not complete yet.
        vm.update_voice_state_event(&make_voice_state(
            guild(1),
            Some(channel(10)),
            user(100),
            "sess-1",
            false,
            true,
        ));
        assert!(vm.connection_info(guild(1)).is_none());

        // Server arrives second -> correlated!
        vm.update_voice_server_event(&make_voice_server(
            guild(1),
            "tok-1",
            Some("eu-central1.discord.media:443"),
        ));

        let info = vm.connection_info(guild(1)).unwrap();
        assert_eq!(info.guild_id, guild(1));
        assert_eq!(info.channel_id, channel(10));
        assert_eq!(&*info.session_id, "sess-1");
        assert_eq!(&*info.token, "tok-1");
        assert_eq!(&*info.endpoint, "eu-central1.discord.media:443");
        assert!(info.self_deaf);

        let ev = events.recv().await.unwrap();
        assert_eq!(ev, VoiceEvent::Connected(info.clone()));

        // Lavalink v4 payload serialization check.
        let lava = info.to_lavalink();
        let lava_json = serde_json::to_value(&lava).unwrap();
        assert_eq!(
            lava_json,
            serde_json::json!({
                "token": "tok-1",
                "endpoint": "eu-central1.discord.media:443",
                "sessionId": "sess-1",
                "channelId": "10"
            })
        );
    }

    #[tokio::test]
    async fn correlate_server_then_state() {
        let gw = Arc::new(MockGateway::default());
        let vm = VoiceManager::with_user_id(gw, user(100));
        let mut events = vm.subscribe();

        // Server arrives first.
        vm.update_voice_server_event(&make_voice_server(
            guild(2),
            "tok-2",
            Some("us-east1.discord.media:443"),
        ));
        assert!(vm.connection_info(guild(2)).is_none());

        // State arrives second.
        vm.update_voice_state_event(&make_voice_state(
            guild(2),
            Some(channel(20)),
            user(100),
            "sess-2",
            true,
            false,
        ));

        let info = vm.connection_info(guild(2)).unwrap();
        assert_eq!(info.channel_id, channel(20));
        assert_eq!(&*info.session_id, "sess-2");
        assert_eq!(&*info.token, "tok-2");
        assert!(info.self_mute);

        let ev = events.recv().await.unwrap();
        assert_eq!(ev, VoiceEvent::Connected(info));
    }

    #[tokio::test]
    async fn server_switch_and_reallocation() {
        let gw = Arc::new(MockGateway::default());
        let vm = VoiceManager::with_user_id(gw, user(100));
        let mut events = vm.subscribe();

        vm.update_voice_state_event(&make_voice_state(
            guild(3),
            Some(channel(30)),
            user(100),
            "sess-3",
            false,
            false,
        ));
        vm.update_voice_server_event(&make_voice_server(guild(3), "tok-old", Some("old.media")));
        let _ = events.recv().await.unwrap();

        // Server switches directly.
        vm.update_voice_server_event(&make_voice_server(guild(3), "tok-new", Some("new.media")));
        let ev = events.recv().await.unwrap();
        match ev {
            VoiceEvent::ServerUpdated(info) => {
                assert_eq!(&*info.token, "tok-new");
                assert_eq!(&*info.endpoint, "new.media");
            }
            other => panic!("expected ServerUpdated, got {other:?}"),
        }

        // Endpoint becomes null (reallocating).
        vm.update_voice_server_event(&make_voice_server(guild(3), "tok-new", None));
        assert!(vm.connection_info(guild(3)).is_none());
        assert_eq!(
            events.recv().await.unwrap(),
            VoiceEvent::EndpointReallocating { guild_id: guild(3) }
        );
    }

    #[tokio::test]
    async fn leave_and_disconnect_clears_state() {
        let gw = Arc::new(MockGateway::default());
        let vm = VoiceManager::with_user_id(Arc::clone(&gw) as Arc<dyn VoiceGateway>, user(100));
        let mut events = vm.subscribe();

        vm.update_voice_state_event(&make_voice_state(
            guild(4),
            Some(channel(40)),
            user(100),
            "sess-4",
            false,
            false,
        ));
        vm.update_voice_server_event(&make_voice_server(guild(4), "tok-4", Some("ep-4")));
        let _ = events.recv().await.unwrap();
        assert!(vm.connection_info(guild(4)).is_some());

        vm.leave(guild(4)).await.unwrap();
        assert_eq!(gw.sent().last().unwrap().channel_id, None);

        // Discord confirms disconnect with channel_id: None.
        vm.update_voice_state_event(&make_voice_state(
            guild(4),
            None,
            user(100),
            "sess-4",
            false,
            false,
        ));
        assert!(vm.connection_info(guild(4)).is_none());
        assert_eq!(
            events.recv().await.unwrap(),
            VoiceEvent::Disconnected { guild_id: guild(4) }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_connection_succeeds_and_times_out() {
        let gw = Arc::new(MockGateway::default());
        let vm = Arc::new(VoiceManager::with_user_id(gw, user(100)));

        // Timeout when nothing arrives.
        let err = vm
            .wait_for_connection(guild(5), Duration::from_secs(2))
            .await
            .unwrap_err();
        assert!(matches!(err, VoiceError::Timeout { guild_id } if guild_id == guild(5)));

        // Succeeds when events arrive concurrently.
        let vm_bg = Arc::clone(&vm);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            vm_bg.update_voice_state_event(&make_voice_state(
                guild(5),
                Some(channel(50)),
                user(100),
                "sess-5",
                false,
                false,
            ));
            tokio::time::sleep(Duration::from_millis(50)).await;
            vm_bg.update_voice_server_event(&make_voice_server(guild(5), "tok-5", Some("ep-5")));
        });

        let info = vm
            .wait_for_connection(guild(5), Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(info.channel_id, channel(50));
        assert_eq!(&*info.endpoint, "ep-5");
    }

    #[tokio::test]
    async fn ignores_other_users_and_tracks_multiple_guilds() {
        let gw = Arc::new(MockGateway::default());
        let vm = VoiceManager::with_user_id(gw, user(100));

        // Another user's voice state is ignored.
        vm.update_voice_state_event(&make_voice_state(
            guild(10),
            Some(channel(101)),
            user(999),
            "other-sess",
            false,
            false,
        ));
        vm.update_voice_server_event(&make_voice_server(guild(10), "tok-10", Some("ep-10")));
        assert!(vm.connection_info(guild(10)).is_none());

        // Two guilds in parallel.
        vm.update_voice_state_event(&make_voice_state(
            guild(10),
            Some(channel(101)),
            user(100),
            "sess-10",
            false,
            false,
        ));
        vm.update_voice_state_event(&make_voice_state(
            guild(20),
            Some(channel(202)),
            user(100),
            "sess-20",
            true,
            true,
        ));
        vm.update_voice_server_event(&make_voice_server(guild(20), "tok-20", Some("ep-20")));

        assert_eq!(
            vm.connection_info(guild(10)).unwrap().channel_id,
            channel(101)
        );
        assert_eq!(
            vm.connection_info(guild(20)).unwrap().channel_id,
            channel(202)
        );
    }

    #[test]
    fn debug_redacts_token() {
        let info = VoiceConnectionInfo {
            guild_id: guild(1),
            channel_id: channel(2),
            session_id: "sess".into(),
            token: "super-secret-token".into(),
            endpoint: "ep".into(),
            self_mute: false,
            self_deaf: false,
        };
        let dbg = format!("{info:?}");
        assert!(!dbg.contains("super-secret-token"));
        assert!(dbg.contains("***"));

        let lava_dbg = format!("{:?}", info.to_lavalink());
        assert!(!lava_dbg.contains("super-secret-token"));
        assert!(lava_dbg.contains("***"));
    }
}
