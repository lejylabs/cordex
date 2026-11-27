//! HTTP transport: hyper + rustls, plus a mockable trait for tests.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use http::{HeaderMap, Request, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::client::legacy::Client as HyperClient;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioTimer};

use crate::error::HttpError;

/// Response as collected by the transport (body already buffered).
#[derive(Debug, Clone)]
pub(crate) struct RawResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Outgoing HTTP. Production uses hyper; tests inject a mock.
pub(crate) trait Transport: Send + Sync + 'static {
    fn send(
        &self,
        request: Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<RawResponse, HttpError>> + Send + '_>>;
}

pub(crate) struct HyperTransport {
    client: HyperClient<hyper_rustls::HttpsConnector<HttpConnector>, Full<Bytes>>,
    max_body: usize,
}

impl HyperTransport {
    pub(crate) fn new(
        connect_timeout: Duration,
        max_body: usize,
        http1_only: bool,
    ) -> Result<Self, HttpError> {
        let mut http = HttpConnector::new();
        http.set_nodelay(true);
        http.set_keepalive(Some(Duration::from_secs(30)));
        http.set_keepalive_interval(Some(Duration::from_secs(10)));
        http.set_keepalive_retries(Some(3));
        http.set_connect_timeout(Some(connect_timeout));
        http.enforce_http(false);

        let tls = hyper_rustls::HttpsConnectorBuilder::new()
            .with_provider_and_platform_verifier(rustls::crypto::aws_lc_rs::default_provider())
            .map_err(HttpError::transport)?
            .https_or_http();

        #[cfg(feature = "http2")]
        let https = if http1_only {
            tls.enable_http1().wrap_connector(http)
        } else {
            tls.enable_all_versions().wrap_connector(http)
        };
        #[cfg(not(feature = "http2"))]
        let https = {
            let _ = http1_only;
            tls.enable_http1().wrap_connector(http)
        };

        let mut builder = HyperClient::builder(TokioExecutor::new());
        builder
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_timer(TokioTimer::new())
            .timer(TokioTimer::new());
        #[cfg(feature = "http2")]
        if !http1_only {
            builder
                .http2_adaptive_window(true)
                .http2_keep_alive_interval(Duration::from_secs(30))
                .http2_keep_alive_while_idle(true);
        }
        let client = builder.build(https);

        Ok(Self { client, max_body })
    }
}

impl Transport for HyperTransport {
    fn send(
        &self,
        request: Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<RawResponse, HttpError>> + Send + '_>> {
        let max_body = self.max_body;
        Box::pin(async move {
            let request = request.map(Full::new);
            let response = self
                .client
                .request(request)
                .await
                .map_err(HttpError::transport)?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = collect_limited(response.into_body(), max_body).await?;
            Ok(RawResponse {
                status,
                headers,
                body,
            })
        })
    }
}

async fn collect_limited(body: Incoming, limit: usize) -> Result<Bytes, HttpError> {
    let mut out = BytesMut::new();
    let mut body = std::pin::pin!(body);
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(HttpError::transport)?;
        if let Ok(data) = frame.into_data() {
            if out.len().saturating_add(data.len()) > limit {
                return Err(HttpError::ResponseTooLarge { limit });
            }
            out.extend_from_slice(&data);
        }
    }
    Ok(out.freeze())
}

/// Scriptable transport for mock REST tests.
pub(crate) struct FnTransport<F> {
    f: F,
}

impl<F> FnTransport<F> {
    pub(crate) fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F, Fut> Transport for FnTransport<F>
where
    F: Fn(Request<Bytes>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<RawResponse, HttpError>> + Send + 'static,
{
    fn send(
        &self,
        request: Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<RawResponse, HttpError>> + Send + '_>> {
        Box::pin((self.f)(request))
    }
}

/// HTTP/1.1 transport over a local Unix Domain Socket (`#[cfg(unix)]`).
///
/// Useful for talking to a local REST proxy or sidecar on the same Linux/Unix
/// host without TCP loopback overhead. Reuses idle keep-alive connections in a
/// bounded pool.
#[cfg(unix)]
pub(crate) struct UnixSocketTransport {
    path: std::path::PathBuf,
    max_body: usize,
    pool:
        std::sync::Arc<std::sync::Mutex<Vec<hyper::client::conn::http1::SendRequest<Full<Bytes>>>>>,
}

#[cfg(unix)]
const MAX_IDLE_UDS_CONNS: usize = 16;

#[cfg(unix)]
impl UnixSocketTransport {
    pub(crate) fn new(path: std::path::PathBuf, max_body: usize) -> Self {
        Self {
            path,
            max_body,
            pool: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[cfg(unix)]
impl Transport for UnixSocketTransport {
    fn send(
        &self,
        request: Request<Bytes>,
    ) -> Pin<Box<dyn Future<Output = Result<RawResponse, HttpError>> + Send + '_>> {
        let path = self.path.clone();
        let max_body = self.max_body;
        let pool = std::sync::Arc::clone(&self.pool);
        Box::pin(async move {
            let mut sender = loop {
                let candidate = pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .pop();
                if let Some(mut s) = candidate {
                    if s.ready().await.is_ok() {
                        break s;
                    }
                } else {
                    let stream = tokio::net::UnixStream::connect(&path)
                        .await
                        .map_err(HttpError::transport)?;
                    let io = hyper_util::rt::TokioIo::new(stream);
                    let (s, conn) = hyper::client::conn::http1::handshake(io)
                        .await
                        .map_err(HttpError::transport)?;
                    tokio::spawn(async move {
                        let _ = conn.await;
                    });
                    break s;
                }
            };
            let response = sender
                .send_request(request.map(Full::new))
                .await
                .map_err(HttpError::transport)?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = collect_limited(response.into_body(), max_body).await?;
            if !sender.is_closed() {
                let mut guard = pool
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if guard.len() < MAX_IDLE_UDS_CONNS {
                    guard.push(sender);
                }
            }
            Ok(RawResponse {
                status,
                headers,
                body,
            })
        })
    }
}
