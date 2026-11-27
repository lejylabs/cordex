//! Persistent session storage for zero-downtime gateway `RESUME` across restarts.
//!
//! Implement [`SessionStore`] to back shard resume state with Redis, Postgres,
//! or another store, or use [`FileSessionStore`] for single-node deployments.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::types::ResumeInfo;

/// Pluggable persistence for shard [`ResumeInfo`] across process restarts.
pub trait SessionStore: Send + Sync + 'static {
    /// Load the last saved resume state for `shard_id`, if any.
    fn load(&self, shard_id: u32) -> Option<ResumeInfo>;

    /// Persist `info` for `shard_id`.
    fn save(&self, shard_id: u32, info: &ResumeInfo) -> io::Result<()>;

    /// Remove any saved resume state for `shard_id`.
    fn clear(&self, shard_id: u32) -> io::Result<()>;
}

/// Directory-backed [`SessionStore`] that writes one JSON file per shard
/// atomically (write to temporary file, then rename).
#[derive(Debug, Clone)]
pub struct FileSessionStore {
    dir: PathBuf,
}

impl FileSessionStore {
    /// Create a store writing `shard-<id>.json` files into `dir`.
    ///
    /// The directory is created on first [`SessionStore::save`] if it does not
    /// already exist.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Directory where shard session files are stored.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path_for(&self, shard_id: u32) -> PathBuf {
        self.dir.join(format!("shard-{shard_id}.json"))
    }

    fn temp_path_for(&self, shard_id: u32) -> PathBuf {
        self.dir.join(format!("shard-{shard_id}.json.tmp"))
    }
}

impl SessionStore for FileSessionStore {
    fn load(&self, shard_id: u32) -> Option<ResumeInfo> {
        let bytes = fs::read(self.path_for(shard_id)).ok()?;
        let val: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let session_id = val.get("session_id")?.as_str()?.into();
        let seq = val.get("seq")?.as_u64()?;
        let resume_gateway_url = val.get("resume_gateway_url")?.as_str()?.into();
        Some(ResumeInfo {
            session_id,
            seq,
            resume_gateway_url,
        })
    }

    fn save(&self, shard_id: u32, info: &ResumeInfo) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let val = serde_json::json!({
            "session_id": info.session_id.as_ref(),
            "seq": info.seq,
            "resume_gateway_url": info.resume_gateway_url.as_ref(),
        });
        let bytes = serde_json::to_vec(&val)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let tmp = self.temp_path_for(shard_id);
        let dst = self.path_for(shard_id);
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &dst)?;
        Ok(())
    }

    fn clear(&self, shard_id: u32) -> io::Result<()> {
        match fs::remove_file(self.path_for(shard_id)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
}

/// Redis-backed [`SessionStore`] for zero-downtime gateway `RESUME` across
/// container or multi-node restarts (`redis` feature).
///
/// Reuses a single Redis connection across calls and transparently reconnects
/// if the connection drops.
#[cfg(feature = "redis")]
#[derive(Clone)]
pub struct RedisSessionStore {
    client: redis::Client,
    conn: std::sync::Arc<parking_lot::Mutex<Option<redis::Connection>>>,
    prefix: Box<str>,
    ttl_secs: Option<u64>,
}

#[cfg(feature = "redis")]
impl std::fmt::Debug for RedisSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisSessionStore")
            .field("prefix", &self.prefix)
            .field("ttl_secs", &self.ttl_secs)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "redis")]
impl RedisSessionStore {
    /// Create a Redis-backed session store from a Redis connection URL
    /// (e.g. `"redis://127.0.0.1/"`).
    pub fn open(url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(url)?;
        Ok(Self::from_client(client))
    }

    /// Create a Redis-backed session store from an existing [`redis::Client`].
    #[must_use]
    pub fn from_client(client: redis::Client) -> Self {
        Self {
            client,
            conn: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            prefix: "cordex:session".into(),
            ttl_secs: Some(86_400),
        }
    }

    /// Customize the Redis key prefix (default: `"cordex:session"`).
    ///
    /// Keys are stored as `<prefix>:<shard_id>`.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<Box<str>>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set a TTL on saved session keys (default: 24 hours). Pass `None` to
    /// persist without expiration.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Option<std::time::Duration>) -> Self {
        self.ttl_secs = ttl.map(|d| d.as_secs().max(1));
        self
    }

    fn key_for(&self, shard_id: u32) -> String {
        format!("{}:{shard_id}", self.prefix)
    }

    fn with_conn<T>(
        &self,
        mut op: impl FnMut(&mut redis::Connection) -> redis::RedisResult<T>,
    ) -> redis::RedisResult<T> {
        let mut guard = self.conn.lock();
        if let Some(conn) = guard.as_mut() {
            match op(conn) {
                Ok(val) => return Ok(val),
                Err(_) => {
                    *guard = None;
                }
            }
        }
        let mut fresh = self.client.get_connection()?;
        let res = op(&mut fresh);
        if res.is_ok() {
            *guard = Some(fresh);
        }
        res
    }
}

#[cfg(feature = "redis")]
impl SessionStore for RedisSessionStore {
    fn load(&self, shard_id: u32) -> Option<ResumeInfo> {
        let key = self.key_for(shard_id);
        let bytes: Option<Vec<u8>> = self
            .with_conn(|conn| redis::Commands::get(conn, &key))
            .ok()?;
        let bytes = bytes?;
        let val: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let session_id = val.get("session_id")?.as_str()?.into();
        let seq = val.get("seq")?.as_u64()?;
        let resume_gateway_url = val.get("resume_gateway_url")?.as_str()?.into();
        Some(ResumeInfo {
            session_id,
            seq,
            resume_gateway_url,
        })
    }

    fn save(&self, shard_id: u32, info: &ResumeInfo) -> io::Result<()> {
        let val = serde_json::json!({
            "session_id": info.session_id.as_ref(),
            "seq": info.seq,
            "resume_gateway_url": info.resume_gateway_url.as_ref(),
        });
        let bytes = serde_json::to_vec(&val)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let key = self.key_for(shard_id);
        let ttl = self.ttl_secs;
        let res: redis::RedisResult<()> = self.with_conn(|conn| match ttl {
            Some(secs) => redis::Commands::set_ex(conn, &key, &bytes, secs),
            None => redis::Commands::set(conn, &key, &bytes),
        });
        res.map_err(|err| io::Error::other(err.to_string()))
    }

    fn clear(&self, shard_id: u32) -> io::Result<()> {
        let key = self.key_for(shard_id);
        let res: redis::RedisResult<()> = self.with_conn(|conn| redis::Commands::del(conn, &key));
        res.map_err(|err| io::Error::other(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        dir.push(format!("cordex-{name}-{pid}-{nanos}"));
        dir
    }

    #[test]
    fn file_session_store_round_trips_and_clears() {
        let dir = temp_dir("session-store");
        let store = FileSessionStore::new(&dir);
        assert_eq!(store.load(0), None);

        let info = ResumeInfo {
            session_id: "sess-123".into(),
            seq: 42,
            resume_gateway_url: "wss://gateway.discord.gg".into(),
        };
        store.save(0, &info).unwrap();
        assert_eq!(store.load(0), Some(info));

        let updated = ResumeInfo {
            session_id: "sess-123".into(),
            seq: 99,
            resume_gateway_url: "wss://gateway.discord.gg".into(),
        };
        store.save(0, &updated).unwrap();
        assert_eq!(store.load(0), Some(updated));

        store.clear(0).unwrap();
        assert_eq!(store.load(0), None);
        // Clearing a missing shard is a no-op.
        store.clear(0).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}
