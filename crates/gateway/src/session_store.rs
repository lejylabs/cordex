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
