//! In-process mock REST client for tests of crates built on this one.
//! No network, no TCP server.

use std::sync::Arc;

use bytes::Bytes;
use http::{HeaderMap, Request, StatusCode};

use crate::client::Client;
use crate::error::HttpError;
use crate::transport::{FnTransport, RawResponse};

/// Canned response of a mock handler.
#[derive(Debug, Clone)]
pub struct MockResponse {
    /// Status code.
    pub status: StatusCode,
    /// Response body (usually JSON).
    pub body: Bytes,
}

impl MockResponse {
    /// `200 OK` with a JSON body.
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: StatusCode::OK,
            body: Bytes::from(body.into()),
        }
    }

    /// `204 No Content`.
    pub fn no_content() -> Self {
        Self {
            status: StatusCode::NO_CONTENT,
            body: Bytes::new(),
        }
    }

    /// Any status with a JSON body.
    pub fn status(status: StatusCode, body: impl Into<String>) -> Self {
        Self {
            status,
            body: Bytes::from(body.into()),
        }
    }
}

/// Client whose requests are answered by `handler` instead of Discord.
pub fn client(
    handler: impl Fn(Request<Bytes>) -> MockResponse + Send + Sync + 'static,
) -> Result<Client, HttpError> {
    Client::builder("mock.token.secret")
        .transport(Arc::new(FnTransport::new(move |req: Request<Bytes>| {
            let resp = handler(req);
            async move {
                Ok(RawResponse {
                    status: resp.status,
                    headers: HeaderMap::new(),
                    body: resp.body,
                })
            }
        })))
        .build()
}
