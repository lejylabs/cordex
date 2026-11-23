#![doc = include_str!("../README.md")]

pub mod endpoint;
pub mod error;
pub mod limiter;
pub mod mock;
pub mod route;

mod client;
mod multipart;
mod request;
mod transport;
mod validate;

pub use client::{AuthScheme, Client, ClientBuilder, DEFAULT_BASE_URL, USER_AGENT};
pub use endpoint::{
    CommandBody, CommandPayload, CreateMessage, EditMessage, ExecuteWebhook, GetChannelMessages,
    InteractionClient,
};
pub use error::{DiscordErrorCode, HttpError, flatten_discord_errors};
pub use limiter::{InProcessRateLimiter, RateLimitScope, RateLimiter};
pub use multipart::{File, Multipart};
pub use request::{RequestBuilder, Response, encode_audit_log_reason};
pub use route::{IntoPathParams, MajorParameters, Route};
pub use validate::{
    CONTENT_LIMIT, EMBED_COUNT_LIMIT, EMBED_TOTAL_LIMIT, FILE_COUNT_LIMIT, FILE_SIZE_LIMIT,
    STICKER_COUNT_LIMIT,
};

pub use http::{HeaderMap, HeaderValue, Method, StatusCode};

#[cfg(test)]
mod tests;
