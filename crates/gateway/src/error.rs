//! Gateway errors (network input never panics).

use std::fmt;
use std::time::Duration;

use cordex_model::gateway::CloseCode;

/// `" (fix: …)"` for close codes with a known fix.
fn hint_suffix(code: CloseCode) -> String {
    code.hint()
        .map_or_else(String::new, |hint| format!(" (fix: {hint})"))
}

/// An error from a shard or gateway helper.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GatewayError {
    /// The shard id is not less than the shard count.
    #[error("invalid shard id {id}/{total}")]
    InvalidShardId {
        /// Requested shard number.
        id: u32,
        /// Shard count.
        total: u32,
    },
    /// Connecting or the WebSocket handshake failed.
    #[error("connect: {0}")]
    Connect(Box<str>),
    /// The socket failed after the handshake.
    #[error("transport: {0}")]
    Transport(Box<str>),
    /// zlib-stream inflate failed or a message exceeded the size cap.
    #[error("zlib: {0}")]
    Zlib(Box<str>),
    /// zstd-stream decompression failed or a message exceeded the size cap.
    #[cfg(feature = "zstd-stream")]
    #[error("zstd: {0}")]
    Zstd(Box<str>),
    /// A gateway payload was not valid JSON.
    #[error("failed to parse gateway payload")]
    Deserialize {
        /// Serde error.
        #[source]
        source: serde_json::Error,
    },
    /// Serializing an outbound payload failed.
    #[error("failed to serialize gateway payload")]
    Serialize {
        /// Serde error.
        #[source]
        source: serde_json::Error,
    },
    /// An outbound payload is larger than 4096 bytes.
    #[error("gateway payload exceeds 4096 bytes")]
    PayloadTooLarge,
    /// Discord closed with a non-reconnectable code.
    #[error("fatal gateway close {code:?}: {reason}{}", hint_suffix(*code))]
    Fatal {
        /// Close code.
        code: CloseCode,
        /// Reason text, if Discord sent one.
        reason: Box<str>,
    },
    /// REST call used by the gateway (`GET /gateway/bot`) failed.
    #[error(transparent)]
    Http(Box<cordex_http::HttpError>),
    /// A local check failed before anything was sent.
    #[error("validation: {0}")]
    Validation(Box<str>),
    /// HELLO, READY/RESUMED or a member-chunk request timed out.
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    /// The shard is stopped and will not take more commands.
    #[error("shard is stopped")]
    Stopped,
    /// The shard task panicked (guarded; the shard reconnects unless this
    /// repeats too often).
    #[error("shard panicked: {0}")]
    Panic(Box<str>),
    /// The shard panicked more than 5 times within 10 minutes and stopped.
    #[error("shard stopped after repeated panics; last: {0}")]
    PanicLimit(Box<str>),
    /// No Tokio runtime to spawn the shard task on. Call `build()` / `start()`
    /// inside a runtime or pass a spawner.
    #[error("no Tokio runtime available to spawn the shard task")]
    NoRuntime,
}

impl GatewayError {
    pub(crate) fn connect(err: impl fmt::Display) -> Self {
        Self::Connect(err.to_string().into_boxed_str())
    }

    pub(crate) fn transport(err: impl fmt::Display) -> Self {
        Self::Transport(err.to_string().into_boxed_str())
    }

    pub(crate) fn zlib(err: impl fmt::Display) -> Self {
        Self::Zlib(err.to_string().into_boxed_str())
    }

    #[cfg(feature = "zstd-stream")]
    pub(crate) fn zstd(err: impl fmt::Display) -> Self {
        Self::Zstd(err.to_string().into_boxed_str())
    }

    pub(crate) fn validation(msg: impl Into<Box<str>>) -> Self {
        Self::Validation(msg.into())
    }

    pub(crate) fn http(err: cordex_http::HttpError) -> Self {
        Self::Http(Box::new(err))
    }

    /// Whether this error ends the shard (no reconnect).
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal { .. } | Self::PanicLimit(_))
    }
}
