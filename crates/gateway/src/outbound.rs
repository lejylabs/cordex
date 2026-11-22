//! Outbound gateway send limiter.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

use crate::types::{OUTBOUND_LIMIT, OUTBOUND_WINDOW, USER_OUTBOUND_LIMIT};

/// Sliding-window limiter: 120 sends / 60 s, 10 reserved for heartbeats.
pub(crate) struct OutboundLimiter {
    sends: VecDeque<Instant>,
}

impl OutboundLimiter {
    pub(crate) fn new() -> Self {
        Self {
            sends: VecDeque::with_capacity(OUTBOUND_LIMIT as usize),
        }
    }

    fn prune(&mut self, now: Instant) {
        let cutoff = now - OUTBOUND_WINDOW;
        while self.sends.front().is_some_and(|t| *t <= cutoff) {
            self.sends.pop_front();
        }
    }

    /// Whether a user payload may be sent now.
    pub(crate) fn can_send_user(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.sends.len() < USER_OUTBOUND_LIMIT as usize
    }

    /// Heartbeats may use the reserved tokens.
    pub(crate) fn can_send_heartbeat(&mut self, now: Instant) -> bool {
        self.prune(now);
        self.sends.len() < OUTBOUND_LIMIT as usize
    }

    /// How long a user payload should wait.
    pub(crate) fn user_wait(&mut self, now: Instant) -> Duration {
        self.prune(now);
        let len = self.sends.len();
        if len < USER_OUTBOUND_LIMIT as usize {
            return Duration::ZERO;
        }
        let drop_needed = len - USER_OUTBOUND_LIMIT as usize + 1;
        let Some(slot) = self.sends.get(drop_needed.saturating_sub(1)).copied() else {
            return Duration::ZERO;
        };
        (slot + OUTBOUND_WINDOW).saturating_duration_since(now)
    }

    pub(crate) fn record(&mut self, now: Instant) {
        self.sends.push_back(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn user_cap_then_wait() {
        let mut lim = OutboundLimiter::new();
        let start = Instant::now();
        for _ in 0..USER_OUTBOUND_LIMIT {
            assert!(lim.can_send_user(Instant::now()));
            lim.record(Instant::now());
        }
        assert!(!lim.can_send_user(Instant::now()));
        assert!(lim.can_send_heartbeat(Instant::now()));
        let wait = lim.user_wait(Instant::now());
        assert!(wait > Duration::ZERO);
        tokio::time::advance(OUTBOUND_WINDOW).await;
        assert!(lim.can_send_user(Instant::now()));
        assert!(Instant::now() >= start + OUTBOUND_WINDOW);
    }
}
