//! Error type for `cordex-test`.

use thiserror::Error;

/// Errors produced while setting up or running an offline [`crate::TestBot`].
#[derive(Debug, Error)]
pub enum TestError {
    /// Building the mock HTTP client failed.
    #[error("mock http error: {0}")]
    Http(#[from] cordex_http::HttpError),
    /// Building the offline shard manager failed.
    #[error("mock gateway error: {0}")]
    Gateway(#[from] cordex_gateway::GatewayError),
    /// Reading a `.jsonl` replay file failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Parsing a `.jsonl` line or event payload failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
