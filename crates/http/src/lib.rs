//! Discord REST client: routes, rate limiter, invalid request guard and
//! retries (PLAN.md §6, §7).

pub mod error;
pub mod limiter;
pub mod route;

mod client;
mod request;
mod transport;

pub use client::{AuthScheme, Client, ClientBuilder, DEFAULT_BASE_URL, USER_AGENT};
pub use error::{DiscordErrorCode, HttpError, flatten_discord_errors};
pub use limiter::{InProcessRateLimiter, RateLimitScope, RateLimiter};
pub use request::{RequestBuilder, Response, encode_audit_log_reason};
pub use route::{IntoPathParams, MajorParameters, Route};

pub use http::{HeaderMap, HeaderValue, Method, StatusCode};

#[cfg(test)]
mod tests;
