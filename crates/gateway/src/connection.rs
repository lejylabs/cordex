//! WebSocket connection trait, production TLS connector, and frames.

use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::OnceLock;

use bytes::Bytes;
use futures_core::Stream;
use futures_sink::Sink;
use http::Uri;
use tokio::net::TcpStream;
use tokio_websockets::{ClientBuilder, Limits};

use crate::error::GatewayError;

/// One WebSocket message from Discord (before zlib).
#[derive(Debug, Clone)]
pub enum Incoming {
    /// Uncompressed JSON text frame.
    Text(Box<str>),
    /// zlib-stream binary frame.
    Binary(Bytes),
    /// Close frame. `None` code is an abnormal close.
    Close {
        /// Discord / WebSocket close code.
        code: Option<u16>,
        /// Close reason.
        reason: Box<str>,
    },
}

/// One WebSocket message we send.
#[derive(Debug, Clone)]
pub enum Outgoing {
    /// JSON text.
    Text(Bytes),
    /// Close the socket.
    Close {
        /// Close code (1000, 4000, …).
        code: u16,
        /// Close reason.
        reason: Box<str>,
    },
}

/// Bidirectional gateway socket.
pub trait Connection: Send {
    /// Next frame, or `None` if the peer dropped the TCP connection.
    fn recv(&mut self) -> impl Future<Output = Result<Option<Incoming>, GatewayError>> + Send;
    /// Send a text or close frame and flush.
    fn send(&mut self, msg: Outgoing) -> impl Future<Output = Result<(), GatewayError>> + Send;
}

/// Opens gateway connections. Tests inject [`crate::mock::MockConnector`].
pub trait Connector: Clone + Send + Sync + 'static {
    /// Connection this connector produces.
    type Connection: Connection;
    /// Connect to a full gateway URL (already includes `v`, `encoding`, `compress`).
    fn connect(
        &self,
        url: &str,
    ) -> impl Future<Output = Result<Self::Connection, GatewayError>> + Send;
}

/// Default cap for one received WebSocket message (16 MiB, PLAN.md §14).
/// zlib-stream frames are compressed, so real frames stay far below it.
pub const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// rustls + platform verifier WebSocket connector (PLAN.md D4, D5).
#[derive(Clone, Debug)]
pub struct WsConnector {
    max_frame_bytes: usize,
}

impl WsConnector {
    /// Connector with the default frame cap ([`DEFAULT_MAX_FRAME_BYTES`]).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }

    /// Largest WebSocket message accepted from Discord; a larger one ends
    /// the connection with a transport error (the shard then resumes).
    #[must_use]
    pub const fn max_frame_bytes(mut self, bytes: usize) -> Self {
        self.max_frame_bytes = bytes;
        self
    }

    /// The configured frame cap.
    #[must_use]
    pub const fn frame_limit(&self) -> usize {
        self.max_frame_bytes
    }
}

impl Default for WsConnector {
    fn default() -> Self {
        Self::new()
    }
}

fn install_rustls_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

fn tls_connector() -> Result<&'static tokio_websockets::Connector, GatewayError> {
    static TLS: OnceLock<tokio_websockets::Connector> = OnceLock::new();
    if let Some(existing) = TLS.get() {
        return Ok(existing);
    }
    install_rustls_provider();
    let created = tokio_websockets::Connector::new().map_err(GatewayError::connect)?;
    Ok(TLS.get_or_init(|| created))
}

impl Connector for WsConnector {
    type Connection = WsConnection;

    async fn connect(&self, url: &str) -> Result<WsConnection, GatewayError> {
        let uri: Uri = url.parse().map_err(GatewayError::connect)?;
        let host = uri
            .host()
            .ok_or_else(|| GatewayError::connect("gateway URL has no host"))?;
        let host_lookup = host.trim_start_matches('[').trim_end_matches(']');
        let port = uri.port_u16().unwrap_or_else(|| match uri.scheme_str() {
            Some("wss") => 443,
            _ => 80,
        });
        let addr = format!("{host_lookup}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(GatewayError::connect)?;
        stream.set_nodelay(true).map_err(GatewayError::connect)?;

        let stream = if uri.scheme_str() == Some("wss") {
            tls_connector()?
                .wrap(host_lookup, stream)
                .await
                .map_err(GatewayError::connect)?
        } else {
            tokio_websockets::Connector::Plain
                .wrap(host_lookup, stream)
                .await
                .map_err(GatewayError::connect)?
        };

        let (ws, _response) = ClientBuilder::from_uri(uri)
            .limits(Limits::default().max_payload_len(Some(self.max_frame_bytes)))
            .connect_on(stream)
            .await
            .map_err(GatewayError::connect)?;
        Ok(WsConnection { stream: ws })
    }
}

/// Production WebSocket connection.
pub struct WsConnection {
    stream: tokio_websockets::WebSocketStream<tokio_websockets::MaybeTlsStream<TcpStream>>,
}

impl WsConnection {
    async fn send_ws(&mut self, message: tokio_websockets::Message) -> Result<(), GatewayError> {
        poll_fn(|cx| Pin::new(&mut self.stream).poll_ready(cx))
            .await
            .map_err(GatewayError::transport)?;
        Pin::new(&mut self.stream)
            .start_send(message)
            .map_err(GatewayError::transport)?;
        poll_fn(|cx| Pin::new(&mut self.stream).poll_flush(cx))
            .await
            .map_err(GatewayError::transport)
    }
}

impl Connection for WsConnection {
    async fn recv(&mut self) -> Result<Option<Incoming>, GatewayError> {
        loop {
            let next = poll_fn(|cx| Pin::new(&mut self.stream).poll_next(cx)).await;
            match next {
                None => return Ok(None),
                Some(Err(err)) => return Err(GatewayError::transport(err)),
                Some(Ok(msg)) => {
                    if msg.is_ping() {
                        let payload = msg.into_payload();
                        self.send_ws(tokio_websockets::Message::pong(payload))
                            .await?;
                        continue;
                    }
                    if msg.is_pong() {
                        continue;
                    }
                    if let Some((code, reason)) = msg.as_close() {
                        return Ok(Some(Incoming::Close {
                            code: Some(u16::from(code)),
                            reason: reason.into(),
                        }));
                    }
                    if let Some(text) = msg.as_text() {
                        return Ok(Some(Incoming::Text(text.into())));
                    }
                    if msg.is_binary() {
                        let bytes = Bytes::copy_from_slice(&msg.as_payload()[..]);
                        return Ok(Some(Incoming::Binary(bytes)));
                    }
                }
            }
        }
    }

    async fn send(&mut self, msg: Outgoing) -> Result<(), GatewayError> {
        match msg {
            Outgoing::Text(bytes) => self.send_ws(tokio_websockets::Message::text(bytes)).await,
            Outgoing::Close { code, reason } => {
                let ws_code = tokio_websockets::CloseCode::try_from(code).ok();
                self.send_ws(tokio_websockets::Message::close(ws_code, &reason))
                    .await?;
                let _ = poll_fn(|cx| Pin::new(&mut self.stream).poll_close(cx)).await;
                Ok(())
            }
        }
    }
}
