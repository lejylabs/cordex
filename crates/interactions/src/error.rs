//! Errors of the interactions crate.

/// Error from command building, responses or the HTTP endpoint.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InteractionError {
    /// A command definition breaks a Discord limit (checked locally).
    #[error("invalid command: {0}")]
    InvalidCommand(Box<str>),
    /// REST request failed.
    #[error(transparent)]
    Http(#[from] drs_http::HttpError),
}
