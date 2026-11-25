//! Lightweight in-memory cooldown tracker for commands and handlers.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const PRUNE_THRESHOLD: usize = 1024;

/// Scope of a command-level cooldown registered via
/// [`Bot::command_with_cooldown`](super::Bot::command_with_cooldown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CooldownScope {
    /// Separate cooldown per invoking user.
    User,
    /// Separate cooldown per channel (falls back to user ID if unavailable).
    Channel,
    /// Separate cooldown per guild (falls back to user ID in DMs).
    Guild,
    /// Single global cooldown shared across all users and guilds.
    Global,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CommandCooldown {
    pub(super) duration: Duration,
    pub(super) scope: CooldownScope,
}

/// Thread-safe, per-key cooldown tracker (e.g. per user ID, guild ID, or
/// `(user_id, command_name)`).
///
/// Store one in [`Bot::data`](super::Bot::data) and check it inside
/// [`Bot::before_command`](super::Bot::before_command) or individual handlers.
#[derive(Debug)]
pub struct Cooldowns<K = u64> {
    entries: Mutex<HashMap<K, Instant>>,
}

impl<K> Default for Cooldowns<K> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash> Cooldowns<K> {
    /// Empty cooldown tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether `key` is currently on cooldown.
    ///
    /// - Returns `None` when the action is allowed (and starts a new cooldown
    ///   for `duration`).
    /// - Returns `Some(remaining)` when `key` must still wait.
    pub fn check(&self, key: K, duration: Duration) -> Option<Duration> {
        let now = Instant::now();
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(&until) = map.get(&key)
            && until > now
        {
            return Some(until.saturating_duration_since(now));
        }
        if map.len() >= PRUNE_THRESHOLD {
            map.retain(|_, until| *until > now);
        }
        map.insert(key, now + duration);
        None
    }

    /// Clear any active cooldown for `key`.
    pub fn reset(&self, key: &K) {
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.remove(key);
    }

    /// Remove all expired entries.
    pub fn prune(&self) {
        let now = Instant::now();
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.retain(|_, until| *until > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_and_resets_cooldown() {
        let cd = Cooldowns::new();
        assert_eq!(cd.check(1_u64, Duration::from_secs(10)), None);
        assert!(cd.check(1_u64, Duration::from_secs(10)).is_some());
        assert_eq!(cd.check(2_u64, Duration::from_secs(10)), None);
        cd.reset(&1_u64);
        assert_eq!(cd.check(1_u64, Duration::from_secs(10)), None);
    }
}
