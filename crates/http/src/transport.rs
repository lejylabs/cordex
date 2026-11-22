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
    pub(crate) fn new(connect_timeout: Duration, max_body: usize) -> Result<Self, HttpError> {
        let mut http = HttpConnector::new();
        http.set_nodelay(true);
        http.set_connect_timeout(Some(connect_timeout));
        http.enforce_http(false);

        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_provider_and_platform_verifier(rustls::crypto::aws_lc_rs::default_provider())
            .map_err(HttpError::transport)?
            .https_or_http()
            .enable_http1()
            .wrap_connector(http);

        let client = HyperClient::builder(TokioExecutor::new())
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_timer(TokioTimer::new())
            .build(https);

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

/// Scriptable transport for mock REST tests (PLAN.md §17.3).
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
