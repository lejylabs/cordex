//! Opt-in gateway dispatch traffic recorder and anonymizer (`record` feature).
//!
//! Records gateway dispatch payloads with relative timestamps as JSON Lines
//! (`.jsonl`) so real production or dev event sequences can be replayed
//! deterministically in `cordex-test`.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde_json::{Map, Value};

/// One recorded gateway dispatch frame in a `.jsonl` capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedFrame {
    /// Milliseconds elapsed since the recorder started.
    pub offset_ms: u64,
    /// Shard number that received the dispatch.
    pub shard: u32,
    /// Discord dispatch event name (`"MESSAGE_CREATE"`, `"INTERACTION_CREATE"`, ...).
    pub event_name: String,
    /// Raw dispatch payload (`d` field of the gateway envelope).
    pub payload: Value,
}

impl RecordedFrame {
    /// Serialize this frame as a single JSON line (without trailing newline).
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        let mut obj = Map::with_capacity(4);
        obj.insert("offset_ms".into(), Value::from(self.offset_ms));
        obj.insert("shard".into(), Value::from(self.shard));
        obj.insert("t".into(), Value::from(self.event_name.as_str()));
        obj.insert("d".into(), self.payload.clone());
        serde_json::to_string(&Value::Object(obj))
    }

    /// Parse one JSON line into a [`RecordedFrame`].
    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        let mut value: Value = serde_json::from_str(line)?;
        let obj = value.as_object_mut();
        let offset_ms = obj
            .as_ref()
            .and_then(|m| m.get("offset_ms"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let shard = obj
            .as_ref()
            .and_then(|m| m.get("shard"))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        let event_name = obj
            .as_ref()
            .and_then(|m| m.get("t"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let payload = obj.and_then(|m| m.remove("d")).unwrap_or(Value::Null);
        Ok(Self {
            offset_ms,
            shard,
            event_name,
            payload,
        })
    }
}

/// Consistent 1:1 anonymizer for gateway JSON payloads.
///
/// Replaces snowflakes, usernames, message contents, tokens, and session IDs
/// consistently across an entire capture so recordings can be committed safely
/// as offline regression fixtures.
#[derive(Debug, Default)]
pub struct Anonymizer {
    snowflakes: HashMap<String, String>,
    users: HashMap<String, String>,
    next_snowflake: u64,
    next_user: u64,
    next_message: u64,
}

impl Anonymizer {
    /// Start a fresh anonymizer with deterministic synthetic IDs.
    pub fn new() -> Self {
        Self {
            snowflakes: HashMap::new(),
            users: HashMap::new(),
            next_snowflake: 1_000_000_000_000_000_001,
            next_user: 1,
            next_message: 1,
        }
    }

    /// Anonymize a JSON value in place.
    pub fn anonymize_value(&mut self, value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    self.anonymize_entry(key, val);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.anonymize_value(item);
                }
            }
            _ => {}
        }
    }

    fn anonymize_entry(&mut self, key: &str, val: &mut Value) {
        match key {
            "token" | "session_id" | "resume_gateway_url" => {
                if val.is_string() {
                    *val = Value::String("<redacted>".to_owned());
                }
            }
            "content" => {
                if let Value::String(s) = val
                    && !s.is_empty()
                {
                    let n = self.next_message;
                    self.next_message += 1;
                    *s = format!("<redacted message {n}>");
                }
            }
            "username" | "global_name" | "nick" => {
                if let Value::String(s) = val
                    && !s.is_empty()
                {
                    *s = self.map_user(s);
                }
            }
            k if k == "id" || k.ends_with("_id") => {
                if let Value::String(s) = val
                    && looks_like_snowflake(s)
                {
                    *s = self.map_snowflake(s);
                } else {
                    self.anonymize_value(val);
                }
            }
            _ => self.anonymize_value(val),
        }
    }

    fn map_snowflake(&mut self, original: &str) -> String {
        if let Some(mapped) = self.snowflakes.get(original) {
            return mapped.clone();
        }
        let mapped = self.next_snowflake.to_string();
        self.next_snowflake += 1;
        self.snowflakes.insert(original.to_owned(), mapped.clone());
        mapped
    }

    fn map_user(&mut self, original: &str) -> String {
        if let Some(mapped) = self.users.get(original) {
            return mapped.clone();
        }
        let mapped = format!("user_{}", self.next_user);
        self.next_user += 1;
        self.users.insert(original.to_owned(), mapped.clone());
        mapped
    }
}

fn looks_like_snowflake(s: &str) -> bool {
    (17..=20).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

enum Sink {
    File(BufWriter<File>),
    Memory(Vec<String>),
}

struct RecorderInner {
    started: Instant,
    sink: Sink,
    anonymizer: Option<Anonymizer>,
}

/// Thread-safe recorder that writes gateway dispatches as `.jsonl`.
#[derive(Clone)]
pub struct TrafficRecorder {
    inner: Arc<Mutex<RecorderInner>>,
}

impl TrafficRecorder {
    /// Open or create a `.jsonl` file for recording.
    pub fn to_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(RecorderInner {
                started: Instant::now(),
                sink: Sink::File(BufWriter::new(file)),
                anonymizer: None,
            })),
        })
    }

    /// Record into an in-memory buffer (useful for tests and inspection).
    pub fn in_memory() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RecorderInner {
                started: Instant::now(),
                sink: Sink::Memory(Vec::new()),
                anonymizer: None,
            })),
        }
    }

    /// Enable consistent 1:1 anonymization for all recorded frames.
    #[must_use]
    pub fn with_anonymizer(self, anonymizer: Anonymizer) -> Self {
        self.inner.lock().anonymizer = Some(anonymizer);
        self
    }

    /// Record one gateway dispatch event.
    pub fn record_dispatch(&self, shard: u32, event_name: &str, payload: &Value) -> io::Result<()> {
        let mut inner = self.inner.lock();
        let offset_ms = u64::try_from(inner.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut payload = payload.clone();
        if let Some(anon) = inner.anonymizer.as_mut() {
            anon.anonymize_value(&mut payload);
        }
        let frame = RecordedFrame {
            offset_ms,
            shard,
            event_name: event_name.to_owned(),
            payload,
        };
        let line = frame
            .to_json_line()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        match &mut inner.sink {
            Sink::File(writer) => {
                writeln!(writer, "{line}")?;
                writer.flush()
            }
            Sink::Memory(lines) => {
                lines.push(line);
                Ok(())
            }
        }
    }

    /// Return all recorded `.jsonl` lines when using [`Self::in_memory`].
    pub fn memory_lines(&self) -> Vec<String> {
        match &self.inner.lock().sink {
            Sink::Memory(lines) => lines.clone(),
            Sink::File(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymizer_is_consistent_and_redacts_secrets() {
        let mut anon = Anonymizer::new();
        let mut first = serde_json::json!({
            "id": "123456789012345678",
            "channel_id": "223456789012345678",
            "content": "secret message",
            "token": "super.secret.token",
            "author": {
                "id": "123456789012345678",
                "username": "alice"
            }
        });
        anon.anonymize_value(&mut first);
        assert_eq!(first["id"], first["author"]["id"]);
        assert_ne!(first["id"], "123456789012345678");
        assert_eq!(first["token"], "<redacted>");
        assert_eq!(first["author"]["username"], "user_1");
        assert_eq!(first["content"], "<redacted message 1>");
    }

    #[test]
    fn recorder_in_memory_roundtrip() {
        let recorder = TrafficRecorder::in_memory().with_anonymizer(Anonymizer::new());
        let res = recorder.record_dispatch(
            0,
            "MESSAGE_CREATE",
            &serde_json::json!({ "id": "123456789012345678", "content": "hi" }),
        );
        assert!(res.is_ok());
        let lines = recorder.memory_lines();
        assert_eq!(lines.len(), 1);
        let parsed = RecordedFrame::from_json_line(&lines[0]);
        assert!(parsed.is_ok());
        if let Ok(frame) = parsed {
            assert_eq!(frame.event_name, "MESSAGE_CREATE");
            assert_eq!(frame.payload["content"], "<redacted message 1>");
        }
    }
}
