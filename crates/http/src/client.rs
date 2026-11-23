//! REST `Client`: auth, User-Agent, pool, timeouts.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cordex_model::Token;
use cordex_model::message::AllowedMentions;
use http::{HeaderValue, Method, Uri};

use crate::error::HttpError;
use crate::limiter::{InProcessRateLimiter, InvalidRequestGuard, RateLimiter};
use crate::request::RequestBuilder;
use crate::route::Route;
use crate::transport::{HyperTransport, Transport};

/// Mandatory Discord User-Agent.
pub const USER_AGENT: &str = concat!(
    "DiscordBot (",
    env!("CARGO_PKG_REPOSITORY"),
    ", ",
    env!("CARGO_PKG_VERSION"),
    ")"
);

/// Default REST base URL including the API version.
pub const DEFAULT_BASE_URL: &str = "https://discord.com/api/v10";

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_BODY: usize = 32 * 1024 * 1024;
const DEFAULT_RETRY_AFTER_CAP: Duration = Duration::from_secs(10);
const DEFAULT_MAX_RETRIES: u32 = 3;

/// How the token is sent in `Authorization`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    /// `Authorization: Bot <token>` (default).
    Bot,
    /// `Authorization: Bearer <token>` (OAuth 2).
    Bearer,
}

/// Cheap-to-clone REST client (internally `Arc`).
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<ClientInner>,
}

pub(crate) struct ClientInner {
    pub token: Token,
    pub auth_scheme: AuthScheme,
    pub authorization: HeaderValue,
    pub user_agent: HeaderValue,
    pub base_url: Uri,
    pub request_timeout: Duration,
    pub retry_after_cap: Duration,
    pub max_retries: u32,
    pub transport: Arc<dyn Transport>,
    pub limiter: Arc<dyn RateLimiter>,
    pub guard: InvalidRequestGuard,
    pub poisoned: AtomicBool,
    pub default_allowed_mentions: Option<AllowedMentions>,
}

impl Client {
    /// Bot client against `https://discord.com/api/v10` with the default limiter.
    pub fn new(token: impl AsRef<str>) -> Result<Self, HttpError> {
        ClientBuilder::new(token).build()
    }

    /// Start a builder (timeouts, base URL, mock transport, custom limiter).
    pub fn builder(token: impl AsRef<str>) -> ClientBuilder {
        ClientBuilder::new(token)
    }

    /// Begin a raw request. The rate limiter keys off `method` + `route`.
    pub fn request(&self, method: Method, mut route: Route) -> RequestBuilder {
        route.method = method;
        RequestBuilder::new(self.clone(), route)
    }

    /// `GET` convenience for [`Client::request`].
    pub fn get(&self, route: Route) -> RequestBuilder {
        self.request(Method::GET, route)
    }

    /// `POST` convenience for [`Client::request`].
    pub fn post(&self, route: Route) -> RequestBuilder {
        self.request(Method::POST, route)
    }

    /// `PUT` convenience for [`Client::request`].
    pub fn put(&self, route: Route) -> RequestBuilder {
        self.request(Method::PUT, route)
    }

    /// `PATCH` convenience for [`Client::request`].
    pub fn patch(&self, route: Route) -> RequestBuilder {
        self.request(Method::PATCH, route)
    }

    /// `DELETE` convenience for [`Client::request`].
    pub fn delete(&self, route: Route) -> RequestBuilder {
        self.request(Method::DELETE, route)
    }

    /// Whether a 401 has poisoned this client.
    pub fn is_poisoned(&self) -> bool {
        self.inner.poisoned.load(Ordering::SeqCst)
    }

    /// Mentions applied when a request has none of its own.
    pub fn default_allowed_mentions(&self) -> Option<&AllowedMentions> {
        self.inner.default_allowed_mentions.as_ref()
    }

    pub(crate) fn fill_allowed_mentions(&self, mentions: &mut Option<AllowedMentions>) {
        if mentions.is_none() {
            mentions.clone_from(&self.inner.default_allowed_mentions);
        }
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("token", &self.inner.token)
            .field("auth_scheme", &self.inner.auth_scheme)
            .field("base_url", &self.inner.base_url)
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}

/// Builder for [`Client`].
pub struct ClientBuilder {
    token: Token,
    auth_scheme: AuthScheme,
    base_url: String,
    user_agent: String,
    connect_timeout: Duration,
    request_timeout: Duration,
    max_body: usize,
    retry_after_cap: Duration,
    max_retries: u32,
    global_rps: u32,
    invalid_warn_at: u32,
    invalid_reject_at: u32,
    limiter: Option<Arc<dyn RateLimiter>>,
    transport: Option<Arc<dyn Transport>>,
    default_allowed_mentions: Option<AllowedMentions>,
}

impl ClientBuilder {
    /// Builder with a bot token (leading `Bot ` is stripped).
    pub fn new(token: impl AsRef<str>) -> Self {
        Self {
            token: Token::new(token.as_ref()),
            auth_scheme: AuthScheme::Bot,
            base_url: DEFAULT_BASE_URL.to_owned(),
            user_agent: USER_AGENT.to_owned(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_body: DEFAULT_MAX_BODY,
            retry_after_cap: DEFAULT_RETRY_AFTER_CAP,
            max_retries: DEFAULT_MAX_RETRIES,
            global_rps: 50,
            invalid_warn_at: crate::limiter::DEFAULT_WARN_AT,
            invalid_reject_at: crate::limiter::DEFAULT_REJECT_AT,
            limiter: None,
            transport: None,
            default_allowed_mentions: None,
        }
    }

    /// Mentions used for messages, follow-ups, webhook executions and
    /// interaction responses that do not set their own.
    #[must_use]
    pub fn default_allowed_mentions(mut self, mentions: AllowedMentions) -> Self {
        self.default_allowed_mentions = Some(mentions);
        self
    }

    /// Use `Bearer` instead of `Bot`.
    #[must_use]
    pub fn bearer(mut self) -> Self {
        self.auth_scheme = AuthScheme::Bearer;
        self
    }

    /// Override the API base URL (proxies and mock servers).
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the User-Agent. Discord requires the `DiscordBot (...)` form
    /// on the production API.
    #[must_use]
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// TCP connect timeout (default 10 s).
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Whole-request timeout including body download (default 15 s).
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Maximum response body size (default 32 MiB).
    #[must_use]
    pub fn max_body_size(mut self, bytes: usize) -> Self {
        self.max_body = bytes;
        self
    }

    /// 429 waits longer than this return [`HttpError::RateLimited`] immediately
    /// (default 10 s).
    #[must_use]
    pub fn retry_after_cap(mut self, cap: Duration) -> Self {
        self.retry_after_cap = cap;
        self
    }

    /// Maximum automatic retries (default 3).
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Global token-bucket rate (default 50/s). Ignored if a custom limiter is set.
    #[must_use]
    pub fn global_rps(mut self, rps: u32) -> Self {
        self.global_rps = rps;
        self
    }

    /// Invalid-request guard thresholds (warn, then local reject). Defaults 8000 / 9500.
    #[must_use]
    pub fn invalid_request_thresholds(mut self, warn_at: u32, reject_at: u32) -> Self {
        self.invalid_warn_at = warn_at;
        self.invalid_reject_at = reject_at;
        self
    }

    /// Replace the in-process limiter (multi-process deployments).
    #[must_use]
    pub fn rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.limiter = Some(limiter);
        self
    }

    /// Inject a transport (tests and [`crate::mock`]). Skips TLS setup.
    #[must_use]
    pub(crate) fn transport(mut self, transport: Arc<dyn Transport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Build the client (installs rustls unless a mock transport was set).
    pub fn build(self) -> Result<Client, HttpError> {
        let authorization = authorization_header(self.auth_scheme, &self.token)?;
        let user_agent = HeaderValue::from_str(&self.user_agent)
            .map_err(|_| HttpError::validation("User-Agent is not a valid header value"))?;
        let base_url: Uri = self
            .base_url
            .parse()
            .map_err(|_| HttpError::validation("base URL is invalid"))?;
        if base_url.scheme().is_none() || base_url.authority().is_none() {
            return Err(HttpError::validation(
                "base URL must include a scheme and authority",
            ));
        }

        let transport = match self.transport {
            Some(t) => t,
            None => Arc::new(HyperTransport::new(self.connect_timeout, self.max_body)?),
        };
        let limiter = self
            .limiter
            .unwrap_or_else(|| Arc::new(InProcessRateLimiter::with_global_rps(self.global_rps)));

        Ok(Client {
            inner: Arc::new(ClientInner {
                token: self.token,
                auth_scheme: self.auth_scheme,
                authorization,
                user_agent,
                base_url,
                request_timeout: self.request_timeout,
                retry_after_cap: self.retry_after_cap,
                max_retries: self.max_retries,
                transport,
                limiter,
                guard: InvalidRequestGuard::new(self.invalid_warn_at, self.invalid_reject_at),
                poisoned: AtomicBool::new(false),
                default_allowed_mentions: self.default_allowed_mentions,
            }),
        })
    }
}

fn authorization_header(scheme: AuthScheme, token: &Token) -> Result<HeaderValue, HttpError> {
    let prefix = match scheme {
        AuthScheme::Bot => "Bot ",
        AuthScheme::Bearer => "Bearer ",
    };
    let mut value = String::with_capacity(prefix.len() + token.expose().len());
    value.push_str(prefix);
    value.push_str(token.expose());
    HeaderValue::from_str(&value)
        .map_err(|_| HttpError::validation("token is not a valid Authorization header value"))
}
