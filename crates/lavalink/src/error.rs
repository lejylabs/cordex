//! Error types for `cordex-lavalink`.

use thiserror::Error;

/// Errors returned by [`crate::LavalinkClient`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LavalinkError {
    /// Invalid node URL or header value.
    #[error("invalid lavalink configuration: {0}")]
    InvalidConfig(Box<str>),
    /// HTTP transport error talking to the Lavalink REST API.
    #[error("lavalink HTTP error: {0}")]
    Http(Box<str>),
    /// Lavalink REST API returned a non-success HTTP status.
    #[error("lavalink returned HTTP {status}: {message}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Response body or reason.
        message: Box<str>,
    },
    /// WebSocket error talking to `/v4/websocket`.
    #[error("lavalink websocket error: {0}")]
    WebSocket(Box<str>),
    /// JSON serialization or deserialization failed.
    #[error("lavalink JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Lavalink has not sent its `ready` event with a `sessionId` yet.
    #[error("lavalink session is not ready yet")]
    NotReady,
    /// Underlying `cordex-voice` error.
    #[error(transparent)]
    Voice(#[from] cordex_voice::VoiceError),
}
