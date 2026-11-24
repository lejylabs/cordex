//! Lavalink v4 WebSocket and REST client.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use cordex_model::id::Id;
use cordex_model::id::marker::GuildMarker;
use cordex_voice::{VoiceConnectionInfo, VoiceEvent, VoiceManager};
use http::{HeaderName, HeaderValue, Method, Request, Uri};
use http_body_util::{BodyExt, Full};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tokio::sync::{broadcast, watch};
use tokio_websockets::ClientBuilder;

use crate::error::LavalinkError;
use crate::model::{LavalinkEvent, LoadResult, NodeConfig, Player, UpdatePlayer};

type HttpClient = HyperClient<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// Default broadcast capacity for [`LavalinkClient::subscribe`].
pub const DEFAULT_EVENT_CAPACITY: usize = 64;

/// Pure-Rust Lavalink v4 client (WebSocket session + REST API + `cordex-voice` bridge).
#[derive(Clone)]
pub struct LavalinkClient {
    inner: Arc<Inner>,
}

struct Inner {
    config: NodeConfig,
    http: HttpClient,
    session_id: watch::Sender<Option<Box<str>>>,
    events: broadcast::Sender<LavalinkEvent>,
}

impl LavalinkClient {
    /// Create a Lavalink v4 client and spawn its background `/v4/websocket` task.
    ///
    /// Call [`Self::wait_ready`] if you want to wait until Lavalink sends its
    /// `ready` event and `sessionId`.
    pub fn connect(config: NodeConfig) -> Result<Self, LavalinkError> {
        let mut http_connector = HttpConnector::new();
        http_connector.enforce_http(false);

        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();

        let https = HttpsConnectorBuilder::new()
            .with_tls_config(tls)
            .https_or_http()
            .enable_http1()
            .wrap_connector(http_connector);

        let http = HyperClient::builder(TokioExecutor::new()).build(https);
        let (session_id, _rx) = watch::channel(None);
        let (events, _ev_rx) = broadcast::channel(DEFAULT_EVENT_CAPACITY);

        let inner = Arc::new(Inner {
            config,
            http,
            session_id,
            events,
        });

        let ws_inner = Arc::clone(&inner);
        tokio::spawn(async move {
            ws_loop(ws_inner).await;
        });

        Ok(Self { inner })
    }

    /// Subscribe to all incoming [`LavalinkEvent`]s.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<LavalinkEvent> {
        self.inner.events.subscribe()
    }

    /// Current Lavalink `sessionId`, if `ready` has been received.
    #[must_use]
    pub fn session_id(&self) -> Option<Box<str>> {
        self.inner.session_id.borrow().clone()
    }

    /// Wait until Lavalink sends its `ready` event and returns the `sessionId`.
    pub async fn wait_ready(&self, timeout: Duration) -> Result<Box<str>, LavalinkError> {
        let mut rx = self.inner.session_id.subscribe();
        if let Some(id) = rx.borrow_and_update().clone() {
            return Ok(id);
        }
        let wait = async {
            loop {
                rx.changed().await.map_err(|_| LavalinkError::NotReady)?;
                if let Some(id) = rx.borrow().clone() {
                    return Ok(id);
                }
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| LavalinkError::NotReady)?
    }

    /// Spawn a background task that bridges a [`VoiceManager`] to this Lavalink
    /// node:
    ///
    /// - `VoiceEvent::Connected(info)` & `VoiceEvent::ServerUpdated(info)` ->
    ///   calls [`Self::update_voice`].
    /// - `VoiceEvent::Disconnected { guild_id }` -> calls [`Self::destroy_player`].
    #[must_use]
    pub fn attach_voice(&self, voice: &VoiceManager) -> tokio::task::JoinHandle<()> {
        let client = self.clone();
        let mut rx = voice.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(VoiceEvent::Connected(info) | VoiceEvent::ServerUpdated(info)) => {
                        if let Err(err) = client.update_voice(&info).await {
                            tracing::warn!(
                                guild_id = %info.guild_id,
                                %err,
                                "failed to forward voice update to lavalink"
                            );
                        }
                    }
                    Ok(VoiceEvent::Disconnected { guild_id }) => {
                        if let Err(err) = client.destroy_player(guild_id).await {
                            tracing::debug!(
                                %guild_id,
                                %err,
                                "failed to destroy lavalink player on disconnect"
                            );
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "lavalink voice bridge lagged behind voice events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }

    /// Resolve a track, playlist, or search query via `GET /v4/loadtracks`.
    pub async fn load_tracks(&self, identifier: &str) -> Result<LoadResult, LavalinkError> {
        let encoded = utf8_percent_encode(identifier, NON_ALPHANUMERIC);
        let url = format!(
            "{}/v4/loadtracks?identifier={encoded}",
            self.inner.config.http_base_url()
        );
        self.request_json(Method::GET, &url, None::<&()>).await
    }

    /// Update a guild player via `PATCH /v4/sessions/{sessionId}/players/{guildId}`.
    pub async fn update_player(
        &self,
        guild_id: Id<GuildMarker>,
        update: &UpdatePlayer,
    ) -> Result<Player, LavalinkError> {
        let session_id = self.session_id().ok_or(LavalinkError::NotReady)?;
        let no_replace = if update.no_replace { "true" } else { "false" };
        let url = format!(
            "{}/v4/sessions/{session_id}/players/{guild_id}?noReplace={no_replace}",
            self.inner.config.http_base_url()
        );
        self.request_json(Method::PATCH, &url, Some(update)).await
    }

    /// Forward a [`VoiceConnectionInfo`] to Lavalink (`PATCH .../players/{guildId}`).
    pub async fn update_voice(&self, info: &VoiceConnectionInfo) -> Result<Player, LavalinkError> {
        let update = UpdatePlayer::new().voice(info.to_lavalink());
        self.update_player(info.guild_id, &update).await
    }

    /// Start playing a base64-encoded track in `guild_id`.
    pub async fn play(
        &self,
        guild_id: Id<GuildMarker>,
        encoded_track: impl Into<Box<str>>,
    ) -> Result<Player, LavalinkError> {
        let update = UpdatePlayer::new().encoded_track(encoded_track);
        self.update_player(guild_id, &update).await
    }

    /// Stop the current track in `guild_id`.
    pub async fn stop(&self, guild_id: Id<GuildMarker>) -> Result<Player, LavalinkError> {
        let update = UpdatePlayer::new().stop_track();
        self.update_player(guild_id, &update).await
    }

    /// Pause or resume playback in `guild_id`.
    pub async fn set_paused(
        &self,
        guild_id: Id<GuildMarker>,
        paused: bool,
    ) -> Result<Player, LavalinkError> {
        let update = UpdatePlayer::new().paused(paused);
        self.update_player(guild_id, &update).await
    }

    /// Set player volume (`0..=1000`) in `guild_id`.
    pub async fn set_volume(
        &self,
        guild_id: Id<GuildMarker>,
        volume: u16,
    ) -> Result<Player, LavalinkError> {
        let update = UpdatePlayer::new().volume(volume);
        self.update_player(guild_id, &update).await
    }

    /// Destroy a guild player (`DELETE /v4/sessions/{sessionId}/players/{guildId}`).
    pub async fn destroy_player(&self, guild_id: Id<GuildMarker>) -> Result<(), LavalinkError> {
        let session_id = self.session_id().ok_or(LavalinkError::NotReady)?;
        let url = format!(
            "{}/v4/sessions/{session_id}/players/{guild_id}",
            self.inner.config.http_base_url()
        );
        self.request_empty(Method::DELETE, &url).await
    }

    async fn request_json<B: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        url: &str,
        body: Option<&B>,
    ) -> Result<R, LavalinkError> {
        let bytes = self.send_raw(method, url, body).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn request_empty(&self, method: Method, url: &str) -> Result<(), LavalinkError> {
        let _ = self.send_raw(method, url, None::<&()>).await?;
        Ok(())
    }

    async fn send_raw<B: serde::Serialize>(
        &self,
        method: Method,
        url: &str,
        body: Option<&B>,
    ) -> Result<Bytes, LavalinkError> {
        let uri: Uri = url.parse().map_err(|err: http::uri::InvalidUri| {
            LavalinkError::InvalidConfig(err.to_string().into())
        })?;

        let payload = match body {
            Some(b) => Bytes::from(serde_json::to_vec(b)?),
            None => Bytes::new(),
        };

        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("Authorization", self.inner.config.password.as_ref());

        if body.is_some() {
            req = req.header("Content-Type", "application/json");
        }

        let req = req
            .body(Full::new(payload))
            .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?;

        let resp = self
            .inner
            .http
            .request(req)
            .await
            .map_err(|err| LavalinkError::Http(err.to_string().into()))?;

        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(|err| LavalinkError::Http(err.to_string().into()))?
            .to_bytes();

        if !status.is_success() {
            let message = String::from_utf8_lossy(&bytes)
                .into_owned()
                .into_boxed_str();
            return Err(LavalinkError::Status {
                status: status.as_u16(),
                message,
            });
        }

        Ok(bytes)
    }
}

async fn ws_loop(inner: Arc<Inner>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        match run_ws_once(&inner).await {
            Ok(()) => {
                backoff = Duration::from_secs(1);
            }
            Err(err) => {
                tracing::warn!(%err, ?backoff, "lavalink websocket disconnected; reconnecting");
            }
        }
        let _ = inner.session_id.send(None);
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn run_ws_once(inner: &Inner) -> Result<(), LavalinkError> {
    let uri: Uri = inner
        .config
        .ws_url()
        .parse()
        .map_err(|err: http::uri::InvalidUri| {
            LavalinkError::InvalidConfig(err.to_string().into())
        })?;

    let auth = HeaderValue::from_str(&inner.config.password)
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?;
    let user_id = HeaderValue::from_str(&inner.config.user_id.to_string())
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?;
    let client_name = HeaderValue::from_str(&inner.config.client_name)
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?;

    let (mut ws, _resp) = ClientBuilder::from_uri(uri)
        .add_header(HeaderName::from_static("authorization"), auth)
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?
        .add_header(HeaderName::from_static("user-id"), user_id)
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?
        .add_header(HeaderName::from_static("client-name"), client_name)
        .map_err(|err| LavalinkError::InvalidConfig(err.to_string().into()))?
        .connect()
        .await
        .map_err(|err| LavalinkError::WebSocket(err.to_string().into()))?;

    while let Some(frame) =
        std::future::poll_fn(|cx| futures_core::Stream::poll_next(std::pin::Pin::new(&mut ws), cx))
            .await
    {
        let msg = frame.map_err(|err| LavalinkError::WebSocket(err.to_string().into()))?;
        if let Some(text) = msg.as_text() {
            match serde_json::from_str::<LavalinkEvent>(text) {
                Ok(event) => {
                    if let LavalinkEvent::Ready { session_id, .. } = &event {
                        let _ = inner.session_id.send(Some(session_id.clone()));
                    }
                    let _ = inner.events.send(event);
                }
                Err(err) => {
                    tracing::debug!(%err, "ignoring unrecognized lavalink websocket payload");
                }
            }
        }
    }

    Ok(())
}

impl std::fmt::Debug for LavalinkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LavalinkClient")
            .field("config", &self.inner.config)
            .field("session_id", &self.session_id())
            .finish()
    }
}
