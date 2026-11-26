//! Result types and assertions for offline interaction & event runs.

use cordex_http::Method;
use serde_json::Value;

use crate::recorder::RecordedRequest;

/// Recorded HTTP activity produced by handling one interaction.
#[derive(Debug, Clone)]
pub struct InteractionResult {
    requests: Vec<RecordedRequest>,
}

impl InteractionResult {
    pub(crate) fn new(requests: Vec<RecordedRequest>) -> Self {
        Self { requests }
    }

    /// Wrap a slice of recorded HTTP requests as an [`InteractionResult`].
    pub fn from_requests(requests: Vec<RecordedRequest>) -> Self {
        Self { requests }
    }

    /// All HTTP requests recorded while handling this interaction.
    pub fn requests(&self) -> &[RecordedRequest] {
        &self.requests
    }

    /// Initial interaction replies (callback type 4) and edits to `@original`.
    pub fn replies(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter_map(|req| {
                let json = req.json.as_ref()?;
                if is_callback(req) && json.get("type").and_then(Value::as_u64) == Some(4) {
                    return json.get("data");
                }
                if req.method == Method::PATCH && req.path.contains("/messages/@original") {
                    return Some(json);
                }
                None
            })
            .collect()
    }

    /// Followup messages sent via `POST /webhooks/{app}/{token}`.
    pub fn followups(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter_map(|req| {
                if req.method == Method::POST
                    && req.path.contains("/webhooks/")
                    && !req.path.contains("/messages/")
                {
                    req.json.as_ref()
                } else {
                    None
                }
            })
            .collect()
    }

    /// Edits to `@original` or followup messages.
    pub fn edits(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter_map(|req| {
                if req.method == Method::PATCH && req.path.contains("/webhooks/") {
                    req.json.as_ref()
                } else {
                    None
                }
            })
            .collect()
    }

    /// Assert that at least one reply or edit contains `expected` in its `content`.
    #[track_caller]
    pub fn assert_replied(&self, expected: &str) {
        let matched = self.replies().into_iter().any(|data| {
            data.get("content")
                .and_then(Value::as_str)
                .is_some_and(|c| c.contains(expected))
        });
        assert!(
            matched,
            "expected a reply containing {expected:?}, got requests: {:#?}",
            self.requests
        );
    }

    /// Assert that an interaction reply or defer set the ephemeral flag (`1 << 6 = 64`).
    #[track_caller]
    pub fn assert_ephemeral(&self) {
        let ephemeral = self.requests.iter().any(|req| {
            let Some(json) = req.json.as_ref() else {
                return false;
            };
            let flags = json
                .get("data")
                .and_then(|d| d.get("flags"))
                .or_else(|| json.get("flags"))
                .and_then(Value::as_u64);
            flags.is_some_and(|f| (f & 64) != 0)
        });
        assert!(
            ephemeral,
            "expected an ephemeral response (flags & 64 != 0), got requests: {:#?}",
            self.requests
        );
    }

    /// Assert that the interaction was deferred (callback type 5 or 6).
    #[track_caller]
    pub fn assert_deferred(&self) {
        let deferred = self.requests.iter().any(|req| {
            is_callback(req)
                && req
                    .json
                    .as_ref()
                    .and_then(|j| j.get("type"))
                    .and_then(Value::as_u64)
                    .is_some_and(|t| t == 5 || t == 6)
        });
        assert!(
            deferred,
            "expected a deferred interaction callback (type 5 or 6), got requests: {:#?}",
            self.requests
        );
    }

    /// Assert that the interaction opened a modal with `expected_custom_id` (callback type 9).
    #[track_caller]
    pub fn assert_modal(&self, expected_custom_id: &str) {
        let matched = self.requests.iter().any(|req| {
            is_callback(req)
                && req.json.as_ref().is_some_and(|j| {
                    j.get("type").and_then(Value::as_u64) == Some(9)
                        && j.get("data")
                            .and_then(|d| d.get("custom_id"))
                            .and_then(Value::as_str)
                            == Some(expected_custom_id)
                })
        });
        assert!(
            matched,
            "expected modal {expected_custom_id:?} (callback type 9), got requests: {:#?}",
            self.requests
        );
    }

    /// Assert that an autocomplete response (callback type 8) returned choices with `expected_names`.
    #[track_caller]
    pub fn assert_autocomplete_choices(&self, expected_names: &[&str]) {
        let names: Vec<&str> = self
            .requests
            .iter()
            .filter(|req| is_callback(req))
            .filter_map(|req| req.json.as_ref())
            .filter(|j| j.get("type").and_then(Value::as_u64) == Some(8))
            .filter_map(|j| j.get("data")?.get("choices")?.as_array())
            .flatten()
            .filter_map(|c| c.get("name")?.as_str())
            .collect();
        assert_eq!(
            names, expected_names,
            "autocomplete choices mismatch; requests: {:#?}",
            self.requests
        );
    }
}

/// Recorded HTTP activity produced by handling a non-interaction event (e.g. `MESSAGE_CREATE`).
#[derive(Debug, Clone)]
pub struct EventResult {
    requests: Vec<RecordedRequest>,
}

impl EventResult {
    pub(crate) fn new(requests: Vec<RecordedRequest>) -> Self {
        Self { requests }
    }

    /// All HTTP requests recorded while handling this event.
    pub fn requests(&self) -> &[RecordedRequest] {
        &self.requests
    }

    /// Messages sent via `POST /channels/{id}/messages`.
    pub fn sent_messages(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter_map(|req| {
                if req.method == Method::POST && req.path.contains("/messages") {
                    req.json.as_ref()
                } else {
                    None
                }
            })
            .collect()
    }

    /// Assert that at least one channel message contains `expected` in its `content`.
    #[track_caller]
    pub fn assert_sent_message(&self, expected: &str) {
        let matched = self.sent_messages().into_iter().any(|json| {
            json.get("content")
                .and_then(Value::as_str)
                .is_some_and(|c| c.contains(expected))
        });
        assert!(
            matched,
            "expected a sent message containing {expected:?}, got requests: {:#?}",
            self.requests
        );
    }
}

fn is_callback(req: &RecordedRequest) -> bool {
    req.method == Method::POST
        && req.path.contains("/interactions/")
        && req.path.ends_with("/callback")
}
