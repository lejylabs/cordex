//! Raw request builder, retries and audit-log reason.

use std::sync::atomic::Ordering;
use std::time::Duration;

use bytes::Bytes;
use http::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use http::{HeaderValue, Method, Request, Uri};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::client::Client;
use crate::error::{HttpError, discord_from_body};
use crate::limiter::{InvalidRequestGuard, PermitGuard, RateLimitScope, parse_secs};
use crate::multipart::Multipart;
use crate::route::Route;
use crate::transport::RawResponse;

const MAX_AUDIT_REASON_CHARS: usize = 512;
const AUDIT_REASON_HEADER: &str = "x-audit-log-reason";

/// Fluent builder for one REST call.
pub struct RequestBuilder {
    client: Client,
    route: Route,
    body: Option<Bytes>,
    content_type: Option<HeaderValue>,
    reason: Option<HeaderValue>,
    query: String,
}

impl RequestBuilder {
    pub(crate) fn new(client: Client, route: Route) -> Self {
        Self {
            client,
            route,
            body: None,
            content_type: None,
            reason: None,
            query: String::new(),
        }
    }

    /// JSON body (`Content-Type: application/json`).
    pub fn json<T: Serialize>(mut self, value: &T) -> Result<Self, HttpError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| HttpError::validation(format!("json body: {e}").into_boxed_str()))?;
        self.body = Some(Bytes::from(bytes));
        self.content_type = Some(HeaderValue::from_static("application/json"));
        Ok(self)
    }

    /// Raw body plus `Content-Type`.
    #[must_use]
    pub fn body(mut self, body: Bytes, content_type: &'static str) -> Self {
        self.body = Some(body);
        self.content_type = Some(HeaderValue::from_static(content_type));
        self
    }

    /// Discord multipart body (`payload_json` + `files[n]`).
    pub fn multipart(mut self, form: &Multipart) -> Result<Self, HttpError> {
        let (content_type, body) = form.encode()?;
        self.body = Some(body);
        self.content_type = Some(content_type);
        Ok(self)
    }

    /// Pre-encoded body with its `Content-Type` (e.g. a form with boundary).
    pub(crate) fn encoded(mut self, (content_type, body): (HeaderValue, Bytes)) -> Self {
        self.body = Some(body);
        self.content_type = Some(content_type);
        self
    }

    /// Appends `key=value` query parameters (percent-encoded).
    #[must_use]
    pub fn query(mut self, key: &str, value: impl std::fmt::Display) -> Self {
        if !self.query.is_empty() {
            self.query.push('&');
        }
        self.query.push_str(key);
        self.query.push('=');
        let value = value.to_string();
        self.query
            .extend(utf8_percent_encode(&value, NON_ALPHANUMERIC));
        self
    }

    /// `X-Audit-Log-Reason`, percent-encoded, max 512 characters.
    pub fn reason(mut self, reason: impl AsRef<str>) -> Result<Self, HttpError> {
        self.reason = Some(encode_audit_log_reason(reason.as_ref())?);
        Ok(self)
    }

    pub(crate) fn maybe_reason(self, reason: Option<&str>) -> Result<Self, HttpError> {
        match reason {
            Some(reason) => self.reason(reason),
            None => Ok(self),
        }
    }

    /// Send the request: rate limit, retries, invalid-request guard.
    pub async fn send(self) -> Result<Response, HttpError> {
        execute(self).await
    }

    /// Send and deserialize a JSON body.
    pub async fn send_json<T: DeserializeOwned>(self) -> Result<T, HttpError> {
        self.send().await?.json()
    }

    /// Send and ignore the body (DELETE / 204).
    pub async fn send_empty(self) -> Result<(), HttpError> {
        let _ = self.send().await?;
        Ok(())
    }
}

/// Buffered REST response. Body is already in memory.
#[derive(Debug, Clone)]
pub struct Response {
    status: http::StatusCode,
    headers: http::HeaderMap,
    body: Bytes,
}

impl Response {
    fn from_raw(raw: RawResponse) -> Self {
        Self {
            status: raw.status,
            headers: raw.headers,
            body: raw.body,
        }
    }

    /// HTTP status.
    pub fn status(&self) -> http::StatusCode {
        self.status
    }

    /// Response headers (including rate-limit headers).
    pub fn headers(&self) -> &http::HeaderMap {
        &self.headers
    }

    /// Raw body bytes.
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// Deserialize the body as JSON.
    pub fn json<T: DeserializeOwned>(self) -> Result<T, HttpError> {
        // Parse from `&str` like the gateway does: one deserializer type for
        // all models keeps the binary much smaller (no second `SliceRead`
        // copy of every model's serde code).
        let parsed = match std::str::from_utf8(&self.body) {
            Ok(text) => serde_json::from_str(text),
            Err(_) => Err(<serde_json::Error as serde::de::Error>::custom(
                "response body is not valid UTF-8",
            )),
        };
        parsed.map_err(|source| HttpError::Deserialize {
            body: self.body,
            source,
        })
    }
}

/// Percent-encode an audit-log reason.
pub fn encode_audit_log_reason(reason: &str) -> Result<HeaderValue, HttpError> {
    let chars = reason.chars().count();
    if chars > MAX_AUDIT_REASON_CHARS {
        return Err(HttpError::validation(
            format!("audit log reason is {chars} characters; max {MAX_AUDIT_REASON_CHARS}")
                .into_boxed_str(),
        ));
    }
    let encoded = utf8_percent_encode(reason, NON_ALPHANUMERIC).to_string();
    HeaderValue::from_str(&encoded)
        .map_err(|_| HttpError::validation("audit log reason is not a valid header value"))
}

fn is_idempotent(method: &Method) -> bool {
    method == Method::GET
        || method == Method::HEAD
        || method == Method::PUT
        || method == Method::DELETE
}

fn jittered_backoff(attempt: u32) -> Duration {
    let exp = attempt.min(4);
    let base_ms = (100u64 << exp).min(1600);
    let jitter = fastrand::u64(0..=base_ms / 2);
    Duration::from_millis(base_ms / 2 + jitter)
}

struct RetryAfter {
    wait: Duration,
    global: bool,
    scope: Option<RateLimitScope>,
}

fn parse_429(raw: &RawResponse) -> RetryAfter {
    #[derive(serde::Deserialize)]
    struct Body {
        retry_after: Option<f64>,
        global: Option<bool>,
    }

    let body: Option<Body> = serde_json::from_slice(&raw.body).ok();
    let from_body = body.as_ref().and_then(|b| {
        b.retry_after.and_then(|v| {
            if v.is_finite() && v >= 0.0 {
                Some(Duration::from_secs_f64(v.min(86_400.0)))
            } else {
                None
            }
        })
    });
    let from_header = raw
        .headers
        .get(http::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_secs);

    let wait = from_body.or(from_header).unwrap_or(Duration::from_secs(1));
    let global = body.as_ref().and_then(|b| b.global).unwrap_or_else(|| {
        raw.headers
            .get("x-ratelimit-global")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    });
    let scope = raw
        .headers
        .get("x-ratelimit-scope")
        .and_then(|v| v.to_str().ok())
        .map(RateLimitScope::parse);
    RetryAfter {
        wait,
        global,
        scope,
    }
}

fn join_url(base: &Uri, path: &str, query: &str) -> Result<Uri, HttpError> {
    let mut parts = base.clone().into_parts();
    let base_path = base.path().trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let joined = if query.is_empty() {
        format!("{base_path}/{path}")
    } else {
        format!("{base_path}/{path}?{query}")
    };
    parts.path_and_query = Some(
        joined
            .parse()
            .map_err(|_| HttpError::validation("request path is not a valid URI"))?,
    );
    Uri::from_parts(parts).map_err(|_| HttpError::validation("could not build request URI"))
}

fn build_http_request(
    client: &Client,
    route: &Route,
    body: Option<Bytes>,
    content_type: Option<HeaderValue>,
    reason: Option<HeaderValue>,
    query: &str,
) -> Result<Request<Bytes>, HttpError> {
    let uri = join_url(&client.inner.base_url, route.path(), query)?;
    let mut builder = Request::builder()
        .method(route.method.clone())
        .uri(uri)
        .header(USER_AGENT, client.inner.user_agent.clone())
        .header(AUTHORIZATION, client.inner.authorization.clone());
    if let Some(ct) = content_type {
        builder = builder.header(CONTENT_TYPE, ct);
    }
    if let Some(reason) = reason {
        builder = builder.header(AUDIT_REASON_HEADER, reason);
    }
    builder
        .body(body.unwrap_or_default())
        .map_err(HttpError::transport)
}

async fn execute(builder: RequestBuilder) -> Result<Response, HttpError> {
    let RequestBuilder {
        client,
        route,
        body,
        content_type,
        reason,
        query,
    } = builder;
    let inner = &client.inner;

    if inner.poisoned.load(Ordering::SeqCst) {
        return Err(HttpError::Unauthorized);
    }
    inner.guard.check()?;

    let mut retries = 0u32;
    loop {
        if inner.poisoned.load(Ordering::SeqCst) {
            return Err(HttpError::Unauthorized);
        }
        inner.guard.check()?;

        inner.limiter.acquire(&route).await?;
        let mut permit = PermitGuard::new(&*inner.limiter, &route);

        let request = build_http_request(
            &client,
            &route,
            body.clone(),
            content_type.clone(),
            reason.clone(),
            &query,
        )?;

        let timed =
            tokio::time::timeout(inner.request_timeout, inner.transport.send(request)).await;
        let raw = match timed {
            Err(_) => return Err(HttpError::Timeout),
            Ok(Err(err)) => {
                if is_idempotent(&route.method) && retries < inner.max_retries {
                    retries += 1;
                    let wait = jittered_backoff(retries - 1);
                    tracing::debug!(retries, wait = ?wait, "transport error, retrying");
                    drop(permit);
                    tokio::time::sleep(wait).await;
                    continue;
                }
                return Err(err);
            }
            Ok(Ok(raw)) => raw,
        };

        permit.committed = true;
        inner.limiter.record(&route, &raw.headers, raw.status);
        if matches!(raw.status.as_u16(), 502..=504)
            && raw.headers.get("x-ratelimit-remaining").is_none()
        {
            inner.limiter.release(&route);
        }
        drop(permit);

        if InvalidRequestGuard::is_invalid(raw.status, &raw.headers) {
            inner.guard.record();
        }

        if raw.status.as_u16() == 401 {
            inner.poisoned.store(true, Ordering::SeqCst);
            tracing::warn!("Discord rejected the token (401); client is poisoned");
            return Err(HttpError::Unauthorized);
        }

        if raw.status.as_u16() == 429 {
            let info = parse_429(&raw);
            if retries < inner.max_retries && info.wait <= inner.retry_after_cap {
                retries += 1;
                tracing::debug!(retries, wait = ?info.wait, global = info.global, "429, retrying");
                tokio::time::sleep(info.wait).await;
                continue;
            }
            return Err(HttpError::RateLimited {
                retry_after: info.wait,
                global: info.global,
                scope: info.scope,
            });
        }

        if matches!(raw.status.as_u16(), 502..=504) {
            if is_idempotent(&route.method) && retries < inner.max_retries {
                retries += 1;
                let wait = jittered_backoff(retries - 1);
                tracing::debug!(retries, wait = ?wait, status = %raw.status, "gateway error, retrying");
                tokio::time::sleep(wait).await;
                continue;
            }
            return Err(discord_from_body(raw.status, &raw.body));
        }

        if raw.status.is_client_error() || raw.status.is_server_error() {
            return Err(discord_from_body(raw.status, &raw.body));
        }

        return Ok(Response::from_raw(raw));
    }
}
