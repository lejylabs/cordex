//! In-process mock gateway (PLAN.md §17.2). No listening TCP server.

use bytes::Bytes;
use tokio::sync::mpsc;

use crate::connection::{Connector, Incoming, Outgoing};
use crate::error::GatewayError;
use crate::zlib::compress_json;

/// Cloneable connector: each [`Connector::connect`] yields a new pair.
#[derive(Clone)]
pub struct MockConnector {
    connects: mpsc::UnboundedSender<MockConnect>,
}

/// Server half of a mock connection, plus the URL the shard used.
pub struct MockConnect {
    /// Full URL including `v`, `encoding` and `compress`.
    pub url: String,
    /// Send frames to the shard and read what it sends.
    pub server: MockServer,
}

/// Scripted Discord side of one socket.
pub struct MockServer {
    tx: mpsc::UnboundedSender<Incoming>,
    rx: mpsc::UnboundedReceiver<Outgoing>,
}

struct MockConnection {
    tx: mpsc::UnboundedSender<Outgoing>,
    rx: mpsc::UnboundedReceiver<Incoming>,
}

impl MockConnector {
    /// Connector plus a channel of accepted connections.
    #[must_use]
    pub fn pair() -> (Self, mpsc::UnboundedReceiver<MockConnect>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { connects: tx }, rx)
    }
}

impl Connector for MockConnector {
    type Connection = impl_conn::Handle;

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "trait signature is async; the mock completes immediately"
    )]
    async fn connect(&self, url: &str) -> Result<impl_conn::Handle, GatewayError> {
        let (to_client, from_server) = mpsc::unbounded_channel();
        let (to_server, from_client) = mpsc::unbounded_channel();
        self.connects
            .send(MockConnect {
                url: url.to_owned(),
                server: MockServer {
                    tx: to_client,
                    rx: from_client,
                },
            })
            .map_err(|_| GatewayError::connect("mock connector dropped"))?;
        Ok(impl_conn::Handle(MockConnection {
            tx: to_server,
            rx: from_server,
        }))
    }
}

/// Hide the connection type so tests talk to [`MockServer`] only.
mod impl_conn {
    use super::MockConnection;
    use crate::connection::{Connection, Incoming, Outgoing};
    use crate::error::GatewayError;

    pub struct Handle(pub(super) MockConnection);

    impl Connection for Handle {
        async fn recv(&mut self) -> Result<Option<Incoming>, GatewayError> {
            Ok(self.0.rx.recv().await)
        }

        #[allow(
            clippy::unused_async_trait_impl,
            reason = "trait signature is async; the mock completes immediately"
        )]
        async fn send(&mut self, msg: Outgoing) -> Result<(), GatewayError> {
            self.0
                .tx
                .send(msg)
                .map_err(|_| GatewayError::transport("mock server dropped"))
        }
    }
}

impl MockServer {
    /// Send an uncompressed JSON text frame.
    pub fn send_json(&self, json: impl Into<String>) {
        let json = json.into();
        let _ = self.tx.send(Incoming::Text(json.into_boxed_str()));
    }

    /// Send JSON as a zlib-stream binary frame.
    pub fn send_zlib(&self, json: &str) -> Result<(), GatewayError> {
        let bin = compress_json(json.as_bytes())?;
        let _ = self.tx.send(Incoming::Binary(Bytes::from(bin)));
        Ok(())
    }

    /// Send a close frame.
    pub fn send_close(&self, code: u16, reason: &str) {
        let _ = self.tx.send(Incoming::Close {
            code: Some(code),
            reason: reason.into(),
        });
    }

    /// Drop the socket without a close frame (TCP reset).
    pub fn drop_tcp(self) {
        drop(self.tx);
    }

    /// Next payload the shard sent, or `None` if the shard dropped.
    pub async fn recv(&mut self) -> Option<Outgoing> {
        self.rx.recv().await
    }

    /// Next JSON text the shard sent.
    pub async fn recv_json(&mut self) -> Option<String> {
        match self.recv().await? {
            Outgoing::Text(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            Outgoing::Close { .. } => None,
        }
    }
}
