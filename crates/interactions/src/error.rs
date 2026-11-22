//! Errors of the interactions crate.

/// Error from command building, responses or the HTTP endpoint.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InteractionError {
    /// A command definition breaks a Discord limit (checked locally).
    #[error("invalid command: {0}")]
    InvalidCommand(Box<str>),
    /// An option is missing or has the wrong type.
    #[error("invalid option: {0}")]
    InvalidOption(Box<str>),
    /// A response breaks a Discord limit (checked locally).
    #[error("invalid response: {0}")]
    InvalidResponse(Box<str>),
    /// The interaction already got its initial response.
    #[error("the interaction was already answered; use edit() or followup()")]
    AlreadyResponded,
    /// Edit/follow-up/delete before any response or defer.
    #[error("the interaction has no response yet; call respond() or defer() first")]
    NotResponded,
    /// A Discord deadline has passed (3 s initial response, 15 min token).
    #[error("interaction expired: {0}")]
    Expired(&'static str),
    /// REST request failed.
    #[error(transparent)]
    Http(#[from] cordex_http::HttpError),
}
