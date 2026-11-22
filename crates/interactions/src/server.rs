//! HTTP interaction endpoint on hyper, without tower.
//!
//! - Ed25519 check on the raw body before JSON parsing; failure → `401`.
//! - `PING` → `PONG` without calling the handler.
//! - Body cap (default 1 MiB) → `413`, handler deadline (default 3 s) → `503`,
//!   stale timestamps (default older than 5 min) → `401`.
//!
//! ```no_run
//! use cordex_interactions::server::InteractionEndpoint;
//! use cordex_interactions::verify::PublicKey;
//! use cordex_model::interaction::InteractionResponse;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let key = PublicKey::from_hex("ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c")?;
//! let endpoint = InteractionEndpoint::new(key, |_interaction| async {
//!     InteractionResponse::message("Hello from HTTP!")
//! });
//! let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
//! endpoint.serve(listener).await;
//! # Ok(())
//! # }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use cordex_model::interaction::{Interaction, InteractionResponse, InteractionType};
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Body;
use hyper_util::rt::TokioIo;

use crate::verify::{PublicKey, SIGNATURE_HEADER, TIMESTAMP_HEADER, verify_signature};

/// Default request body cap.
pub const DEFAULT_MAX_BODY: usize = 1024 * 1024;
/// Default deadline for the handler (Discord waits 3 s).
pub const DEFAULT_HANDLER_TIMEOUT: Duration = Duration::from_secs(3);
/// Default max. age of `X-Signature-Timestamp` (replay protection).
pub const DEFAULT_MAX_TIMESTAMP_AGE: Duration = Duration::from_mins(5);

type BoxFuture = Pin<Box<dyn Future<Output = InteractionResponse> + Send>>;
type Handler = dyn Fn(Interaction) -> BoxFuture + Send + Sync;

/// Verifying endpoint that turns requests into [`Interaction`]s and the
/// handler's [`InteractionResponse`] into the HTTP reply. Cheap to clone.
#[derive(Clone)]
pub struct InteractionEndpoint {
    inner: Arc<Inner>,
}

struct Inner {
    key: PublicKey,
    handler: Box<Handler>,
    max_body: usize,
    timeout: Duration,
    max_age: Option<Duration>,
}

impl std::fmt::Debug for InteractionEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractionEndpoint")
            .field("max_body", &self.inner.max_body)
            .field("timeout", &self.inner.timeout)
            .finish_non_exhaustive()
    }
}

/// Builder for [`InteractionEndpoint`].
pub struct InteractionEndpointBuilder {
    inner: Inner,
}

impl InteractionEndpointBuilder {
    /// Request body cap in bytes (default 1 MiB).
    #[must_use]
    pub fn max_body(mut self, bytes: usize) -> Self {
        self.inner.max_body = bytes;
        self
    }

    /// Handler deadline (default 3 s); on timeout the reply is `503`.
    #[must_use]
    pub fn handler_timeout(mut self, timeout: Duration) -> Self {
        self.inner.timeout = timeout;
        self
    }

    /// Max. age of the signed timestamp (default 5 min); `None` disables
    /// the check.
    #[must_use]
    pub fn max_timestamp_age(mut self, age: Option<Duration>) -> Self {
        self.inner.max_age = age;
        self
    }

    /// Finish the endpoint.
    pub fn build(self) -> InteractionEndpoint {
        InteractionEndpoint {
            inner: Arc::new(self.inner),
        }
    }
}

impl InteractionEndpoint {
    /// Endpoint with default limits; see [`Self::builder`].
    pub fn new<F, Fut>(key: PublicKey, handler: F) -> Self
    where
        F: Fn(Interaction) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = InteractionResponse> + Send + 'static,
    {
        Self::builder(key, handler).build()
    }

    /// Builder for `key` calling `handler` for every verified interaction
    /// except `PING`.
    pub fn builder<F, Fut>(key: PublicKey, handler: F) -> InteractionEndpointBuilder
    where
        F: Fn(Interaction) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = InteractionResponse> + Send + 'static,
    {
        InteractionEndpointBuilder {
            inner: Inner {
                key,
                handler: Box::new(move |i| Box::pin(handler(i))),
                max_body: DEFAULT_MAX_BODY,
                timeout: DEFAULT_HANDLER_TIMEOUT,
                max_age: Some(DEFAULT_MAX_TIMESTAMP_AGE),
            },
        }
    }

    /// Handle one request (any hyper-compatible body). Use this to mount
    /// the endpoint inside another server.
    pub async fn handle<B>(&self, req: Request<B>) -> Response<Full<Bytes>>
    where
        B: Body + Send,
        B::Data: Send,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        if req.method() != Method::POST {
            return status(StatusCode::METHOD_NOT_ALLOWED, "POST only");
        }
        let (parts, body) = req.into_parts();
        let header_str = |name: &str| parts.headers.get(name).and_then(|v| v.to_str().ok());
        let (Some(signature), Some(timestamp)) =
            (header_str(SIGNATURE_HEADER), header_str(TIMESTAMP_HEADER))
        else {
            return status(StatusCode::UNAUTHORIZED, "missing signature headers");
        };
        let (signature, timestamp) = (signature.to_owned(), timestamp.to_owned());

        let body = match Limited::new(body, self.inner.max_body).collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err)
                if err
                    .downcast_ref::<http_body_util::LengthLimitError>()
                    .is_some() =>
            {
                return status(StatusCode::PAYLOAD_TOO_LARGE, "body too large");
            }
            Err(_) => return status(StatusCode::BAD_REQUEST, "body read failed"),
        };

        if verify_signature(&self.inner.key, &signature, &timestamp, &body).is_err() {
            return status(StatusCode::UNAUTHORIZED, "invalid request signature");
        }
        if let Some(max_age) = self.inner.max_age
            && !timestamp_fresh(&timestamp, max_age)
        {
            return status(StatusCode::UNAUTHORIZED, "stale request timestamp");
        }

        let interaction: Interaction = match serde_json::from_slice(&body) {
            Ok(i) => i,
            Err(err) => {
                tracing::warn!(%err, "interaction deserialize failed");
                return status(StatusCode::BAD_REQUEST, "invalid interaction");
            }
        };
        let response = if interaction.kind == InteractionType::Ping {
            InteractionResponse::pong()
        } else {
            let fut = (self.inner.handler)(interaction);
            match tokio::time::timeout(self.inner.timeout, fut).await {
                Ok(r) => r,
                Err(_) => {
                    return status(StatusCode::SERVICE_UNAVAILABLE, "handler timed out");
                }
            }
        };
        match serde_json::to_vec(&response) {
            Ok(json) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(json)))
                .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR, "response")),
            Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR, "response encode failed"),
        }
    }

    /// Accept HTTP/1.1 connections on `listener` until the task is dropped.
    /// Put TLS (Discord requires HTTPS) in front, e.g. a reverse proxy.
    pub async fn serve(self, listener: tokio::net::TcpListener) {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(conn) => conn,
                Err(err) => {
                    tracing::warn!(%err, "accept failed");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            let endpoint = self.clone();
            tokio::spawn(async move {
                let service = hyper::service::service_fn(move |req| {
                    let endpoint = endpoint.clone();
                    async move { Ok::<_, std::convert::Infallible>(endpoint.handle(req).await) }
                });
                if let Err(err) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    tracing::debug!(%err, %peer, "connection error");
                }
            });
        }
    }
}

fn timestamp_fresh(timestamp: &str, max_age: Duration) -> bool {
    let Ok(secs) = timestamp.trim().parse::<u64>() else {
        return false;
    };
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    now.as_secs().abs_diff(secs) <= max_age.as_secs()
}

fn status(code: StatusCode, msg: &'static str) -> Response<Full<Bytes>> {
    let mut resp = Response::new(Full::new(Bytes::from_static(msg.as_bytes())));
    *resp.status_mut() = code;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::tests::{public_key, sign};

    fn now() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    fn builder() -> InteractionEndpointBuilder {
        InteractionEndpoint::builder(public_key(), |i: Interaction| async move {
            InteractionResponse::message(format!("got {}", i.id))
        })
    }

    fn endpoint() -> InteractionEndpoint {
        builder().build()
    }

    fn request(body: &str, ts: &str, sig: Option<String>) -> Request<Full<Bytes>> {
        let mut b = Request::post("/interactions").header(TIMESTAMP_HEADER, ts);
        if let Some(sig) = sig {
            b = b.header(SIGNATURE_HEADER, sig);
        }
        b.body(Full::new(Bytes::from(body.to_owned()))).unwrap()
    }

    async fn body_of(resp: Response<Full<Bytes>>) -> (StatusCode, String) {
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    const PING: &str = r#"{"id":"1","application_id":"2","type":1,"token":"t","version":1}"#;
    const COMMAND: &str = r#"{"id":"42","application_id":"2","type":2,"token":"t","version":1,
        "data":{"id":"3","name":"ping","type":1}}"#;

    #[tokio::test]
    async fn ping_gets_pong() {
        let ts = now();
        let (code, body) = body_of(
            endpoint()
                .handle(request(PING, &ts, Some(sign(&ts, PING.as_bytes()))))
                .await,
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body, r#"{"type":1}"#);
    }

    #[tokio::test]
    async fn command_calls_handler() {
        let ts = now();
        let (code, body) = body_of(
            endpoint()
                .handle(request(COMMAND, &ts, Some(sign(&ts, COMMAND.as_bytes()))))
                .await,
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body, r#"{"type":4,"data":{"content":"got 42"}}"#);
    }

    #[tokio::test]
    async fn bad_or_missing_signature_is_401() {
        let ts = now();
        let wrong = sign(&ts, b"other body");
        let (code, _) = body_of(endpoint().handle(request(PING, &ts, Some(wrong))).await).await;
        assert_eq!(code, StatusCode::UNAUTHORIZED);
        let (code, _) = body_of(endpoint().handle(request(PING, &ts, None)).await).await;
        assert_eq!(code, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn stale_timestamp_is_401_unless_disabled() {
        let ts = "1000000000";
        let sig = sign(ts, PING.as_bytes());
        let (code, body) = body_of(
            endpoint()
                .handle(request(PING, ts, Some(sig.clone())))
                .await,
        )
        .await;
        assert_eq!(
            (code, body.as_str()),
            (StatusCode::UNAUTHORIZED, "stale request timestamp")
        );
        let relaxed = builder().max_timestamp_age(None).build();
        let (code, _) = body_of(relaxed.handle(request(PING, ts, Some(sig))).await).await;
        assert_eq!(code, StatusCode::OK);
    }

    #[tokio::test]
    async fn oversized_body_is_413_and_get_is_405() {
        let ts = now();
        let big = "x".repeat(100);
        let small = builder().max_body(10).build();
        let (code, _) = body_of(
            small
                .handle(request(&big, &ts, Some(sign(&ts, big.as_bytes()))))
                .await,
        )
        .await;
        assert_eq!(code, StatusCode::PAYLOAD_TOO_LARGE);
        let get = Request::get("/").body(Full::new(Bytes::new())).unwrap();
        let (code, _) = body_of(endpoint().handle(get).await).await;
        assert_eq!(code, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test(start_paused = true)]
    async fn slow_handler_is_503() {
        let slow = InteractionEndpoint::new(public_key(), |_| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            InteractionResponse::pong()
        });
        let ts = now();
        let (code, _) = body_of(
            slow.handle(request(COMMAND, &ts, Some(sign(&ts, COMMAND.as_bytes()))))
                .await,
        )
        .await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn serves_over_tcp() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(endpoint().serve(listener));
        let ts = now();
        let sig = sign(&ts, PING.as_bytes());
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "POST / HTTP/1.1\r\nhost: x\r\n{SIGNATURE_HEADER}: {sig}\r\n{TIMESTAMP_HEADER}: {ts}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{PING}",
            PING.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut stream, req.as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut stream, &mut out)
            .await
            .unwrap();
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        assert!(out.ends_with(r#"{"type":1}"#), "{out}");
        server.abort();
    }
}
