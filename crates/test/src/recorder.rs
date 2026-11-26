//! In-memory mock HTTP transport and request recorder.

use std::collections::VecDeque;
use std::sync::Arc;

use bytes::Bytes;
use cordex_http::mock::{self, MockResponse};
use cordex_http::{Client, HttpError, Method, StatusCode};
use parking_lot::Mutex;
use serde_json::Value;

/// One outgoing REST request captured by [`MockHttp`].
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    /// HTTP method (`GET`, `POST`, `PATCH`, `DELETE`, ...).
    pub method: Method,
    /// Request path and query (e.g. `/api/v10/interactions/.../callback`).
    pub path: String,
    /// Raw request body bytes.
    pub body: Bytes,
    /// Parsed JSON body, if the body was valid JSON.
    pub json: Option<Value>,
}

struct MockHttpState {
    requests: Vec<RecordedRequest>,
    overrides: VecDeque<(Option<String>, MockResponse)>,
}

/// In-memory mock HTTP recorder for offline bot tests.
#[derive(Clone)]
pub struct MockHttp {
    state: Arc<Mutex<MockHttpState>>,
}

impl Default for MockHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl MockHttp {
    /// Create a new empty mock HTTP recorder.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockHttpState {
                requests: Vec::new(),
                overrides: VecDeque::new(),
            })),
        }
    }

    /// Build a [`cordex_http::Client`] backed by this recorder.
    pub fn client(&self) -> Result<Client, HttpError> {
        let state = Arc::clone(&self.state);
        mock::client(move |req| {
            let method = req.method().clone();
            let path = req
                .uri()
                .path_and_query()
                .map_or_else(String::new, ToString::to_string);
            let body = req.into_body();
            let json = if body.is_empty() {
                None
            } else {
                serde_json::from_slice::<Value>(&body).ok()
            };

            let mut guard = state.lock();
            guard.requests.push(RecordedRequest {
                method: method.clone(),
                path: path.clone(),
                body,
                json,
            });

            if let Some(idx) = guard.overrides.iter().position(|(filter, _)| {
                filter.as_deref().is_none_or(|needle| path.contains(needle))
            }) && let Some((_, resp)) = guard.overrides.remove(idx)
            {
                return resp;
            }

            default_response(&method, &path)
        })
    }

    /// Queue a custom response for the next matching request.
    pub fn push_response(&self, response: MockResponse) {
        self.state.lock().overrides.push_back((None, response));
    }

    /// Queue a custom response for the next request whose path contains `path_contains`.
    pub fn push_response_for(&self, path_contains: impl Into<String>, response: MockResponse) {
        self.state
            .lock()
            .overrides
            .push_back((Some(path_contains.into()), response));
    }

    /// Inject a Discord JSON error response (e.g. `403 Forbidden` or `429 Too Many Requests`).
    pub fn inject_error(&self, status: StatusCode, code: u32, message: impl Into<String>) {
        let body = serde_json::json!({
            "code": code,
            "message": message.into(),
        });
        self.push_response(MockResponse::status(status, body.to_string()));
    }

    /// Snapshot of all recorded requests so far.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.state.lock().requests.clone()
    }

    /// Current number of recorded requests.
    pub fn len(&self) -> usize {
        self.state.lock().requests.len()
    }

    /// Whether no requests have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.state.lock().requests.is_empty()
    }

    /// Slice of requests recorded starting from `start` index.
    pub fn requests_since(&self, start: usize) -> Vec<RecordedRequest> {
        let guard = self.state.lock();
        guard.requests.get(start..).unwrap_or(&[]).to_vec()
    }

    /// Clear all recorded requests.
    pub fn clear(&self) {
        self.state.lock().requests.clear();
    }
}

fn default_response(method: &Method, path: &str) -> MockResponse {
    if path.contains("/gateway/bot") {
        return MockResponse::json(
            serde_json::json!({
                "url": "wss://gateway.discord.gg",
                "shards": 1,
                "session_start_limit": {
                    "total": 1000,
                    "remaining": 999,
                    "reset_after": 14_400_000,
                    "max_concurrency": 1
                }
            })
            .to_string(),
        );
    }
    if path.contains("/interactions/") && path.ends_with("/callback") {
        return MockResponse::no_content();
    }
    if *method == Method::DELETE {
        return MockResponse::no_content();
    }
    if path.contains("/messages") || path.contains("/webhooks/") {
        return MockResponse::json(minimal_message_json());
    }
    MockResponse::json("{}")
}

fn minimal_message_json() -> String {
    serde_json::json!({
        "id": "900000000000000001",
        "channel_id": "200000000000000001",
        "author": {
            "id": "100000000000000001",
            "username": "test-bot",
            "discriminator": "0",
            "bot": true
        },
        "content": "",
        "timestamp": "2026-01-01T00:00:00.000000+00:00",
        "tts": false,
        "mention_everyone": false,
        "mentions": [],
        "mention_roles": [],
        "attachments": [],
        "embeds": [],
        "pinned": false,
        "type": 0
    })
    .to_string()
}
