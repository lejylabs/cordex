//! Identify queue (PLAN.md §9.3): one Identify per bucket every 5 s, plus
//! the session-start limit from `GET /gateway/bot`.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

use drs_model::gateway::SessionStartLimit;

/// Discord: at most one Identify per bucket every 5 seconds.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(5);
/// Extra delay on top of [`IDENTIFY_INTERVAL`] so the client stays under the
/// limit when clocks are slightly off.
pub const IDENTIFY_MARGIN: Duration = Duration::from_millis(250);
/// Full spacing used by [`LocalIdentifyQueue`] (5 s + 250 ms).
pub const IDENTIFY_SPACING: Duration = Duration::from_millis(5_250);

/// Serializes Identify payloads so Discord's 5-second-per-bucket rule is
/// kept. Resumes do not go through the queue.
pub trait IdentifyQueue: Send + Sync + 'static {
    /// Wait until this shard may send Identify.
    fn acquire(&self, shard_id: u32) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// Queue that grants every identify immediately (single-shard / tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct ImmediateIdentifyQueue;

impl IdentifyQueue for ImmediateIdentifyQueue {
    fn acquire(&self, _shard_id: u32) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

/// In-process identify queue: `shard_id % max_concurrency` buckets, 5.25 s
/// spacing, optional session-start budget.
pub struct LocalIdentifyQueue {
    max_concurrency: u32,
    spacing: Duration,
    buckets: Vec<Mutex<Option<Instant>>>,
    session: Mutex<SessionBudget>,
}

struct SessionBudget {
    unlimited: bool,
    remaining: u32,
    total: u32,
    reset_at: Instant,
    window: Duration,
}

impl LocalIdentifyQueue {
    /// Queue with `max_concurrency` buckets and no session-start cap.
    /// `max_concurrency` of 0 is treated as 1.
    #[must_use]
    pub fn new(max_concurrency: u32) -> Self {
        let n = max_concurrency.max(1) as usize;
        Self {
            max_concurrency: u32::try_from(n).unwrap_or(1),
            spacing: IDENTIFY_SPACING,
            buckets: (0..n).map(|_| Mutex::new(None)).collect(),
            session: Mutex::new(SessionBudget {
                unlimited: true,
                remaining: u32::MAX,
                total: u32::MAX,
                reset_at: Instant::now(),
                window: Duration::from_hours(24),
            }),
        }
    }

    /// Apply the session-start budget from `GET /gateway/bot`.
    #[must_use]
    pub fn with_session_limit(mut self, limit: SessionStartLimit) -> Self {
        let window = Duration::from_millis(limit.reset_after);
        self.session = Mutex::new(SessionBudget {
            unlimited: false,
            remaining: limit.remaining,
            total: limit.total,
            reset_at: Instant::now() + window,
            window,
        });
        self
    }

    /// Override bucket spacing (tests).
    #[must_use]
    pub fn with_spacing(mut self, spacing: Duration) -> Self {
        self.spacing = spacing;
        self
    }

    /// Concurrent identify buckets (`shard_id % max_concurrency`).
    #[must_use]
    pub const fn max_concurrency(&self) -> u32 {
        self.max_concurrency
    }

    async fn acquire_inner(&self, shard_id: u32) {
        self.wait_session().await;
        self.wait_bucket(shard_id).await;
    }

    async fn wait_session(&self) {
        loop {
            let mut budget = self.session.lock().await;
            if budget.unlimited {
                return;
            }
            let now = Instant::now();
            if now >= budget.reset_at {
                budget.remaining = budget.total.max(1);
                budget.reset_at = now + budget.window;
            }
            if budget.remaining > 0 {
                budget.remaining -= 1;
                if budget.remaining == 0 {
                    tracing::warn!(
                        reset_in = ?budget.reset_at.saturating_duration_since(Instant::now()),
                        "session start limit remaining is 0; further identifies wait"
                    );
                }
                return;
            }
            let wait = budget.reset_at.saturating_duration_since(now);
            tracing::warn!(
                ?wait,
                "session start limit exhausted, waiting for reset_after"
            );
            drop(budget);
            if wait.is_zero() {
                sleep(Duration::from_millis(1)).await;
            } else {
                sleep(wait).await;
            }
        }
    }

    async fn wait_bucket(&self, shard_id: u32) {
        let index = (shard_id % self.max_concurrency) as usize;
        let mut last = self.buckets[index].lock().await;
        if let Some(granted) = *last {
            let earliest = granted + self.spacing;
            let now = Instant::now();
            if now < earliest {
                sleep(earliest - now).await;
            }
        }
        *last = Some(Instant::now());
    }
}

impl IdentifyQueue for LocalIdentifyQueue {
    fn acquire(&self, shard_id: u32) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.acquire_inner(shard_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn concurrency_one_spaces_identifies() {
        let queue = LocalIdentifyQueue::new(1);
        let t0 = Instant::now();
        queue.acquire(0).await;
        queue.acquire(1).await;
        let elapsed = t0.elapsed();
        assert!(
            elapsed >= IDENTIFY_SPACING,
            "second identify came after {elapsed:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn concurrency_sixteen_allows_parallel_buckets() {
        let queue = LocalIdentifyQueue::new(16);
        let t0 = Instant::now();
        tokio::join!(queue.acquire(0), queue.acquire(1));
        assert!(
            t0.elapsed() < Duration::from_millis(50),
            "different buckets must not wait on each other"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn session_remaining_zero_waits_reset_after() {
        let queue = LocalIdentifyQueue::new(16)
            .with_session_limit(SessionStartLimit::new(1000, 0, 1_000, 16));
        let t0 = Instant::now();
        queue.acquire(0).await;
        let elapsed = t0.elapsed();
        assert!(
            elapsed >= Duration::from_millis(1_000),
            "identify with remaining=0 waited {elapsed:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn session_remaining_one_then_wait() {
        let queue = LocalIdentifyQueue::new(16)
            .with_session_limit(SessionStartLimit::new(1000, 1, 2_000, 16));
        let t0 = Instant::now();
        queue.acquire(0).await;
        assert!(t0.elapsed() < Duration::from_millis(50));
        queue.acquire(1).await;
        assert!(t0.elapsed() >= Duration::from_millis(2_000));
    }
}
