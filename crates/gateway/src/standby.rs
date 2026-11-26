//! Wait for the next matching gateway event (Twilight's `standby`).
//!
//! Registered waiters are checked in [`Standby::process`]; the event is
//! cloned only when a waiter matches. Waiters whose receiver is gone
//! (timed out, dropped) are removed on the next event, whether or not it
//! matches, so the list never grows without bound. Predicates run under a
//! short lock: keep them cheap and non-blocking.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use cordex_model::gateway::Event;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

type Pred = Arc<dyn Fn(&Event) -> bool + Send + Sync>;

/// Buffer of a [`Standby::wait_for_stream`] receiver; events beyond it are
/// dropped (with a warning) instead of growing memory.
pub const STANDBY_STREAM_CAPACITY: usize = 256;

enum Sink {
    Once(oneshot::Sender<Event>),
    Stream(mpsc::Sender<Event>),
}

impl Sink {
    fn is_closed(&self) -> bool {
        match self {
            Self::Once(tx) => tx.is_closed(),
            Self::Stream(tx) => tx.is_closed(),
        }
    }
}

struct Waiter {
    id: u64,
    pred: Pred,
    sink: Sink,
}

#[derive(Default)]
struct StandbyInner {
    waiters: Mutex<Vec<Waiter>>,
    count: AtomicUsize,
    next_id: AtomicU64,
}

/// Removes a `wait_for` waiter from [`StandbyInner`] as soon as the future is
/// dropped (cancelled in `tokio::select!`, timed out, or unwound on panic),
/// preventing zombie entries from lingering until the next gateway event.
struct WaiterGuard {
    inner: Arc<StandbyInner>,
    id: u64,
}

impl Drop for WaiterGuard {
    fn drop(&mut self) {
        let mut guard = self.inner.waiters.lock();
        if let Some(pos) = guard.iter().position(|w| w.id == self.id) {
            guard.swap_remove(pos);
            self.inner.count.store(guard.len(), Ordering::Release);
        }
    }
}

/// Shared list of event waiters. Cheap to clone (`Arc`).
#[derive(Clone, Default)]
pub struct Standby {
    inner: Arc<StandbyInner>,
}

impl Standby {
    /// Empty waiter list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anyone is waiting.
    pub fn is_empty(&self) -> bool {
        self.inner.count.load(Ordering::Acquire) == 0
    }

    /// Number of currently registered event waiters.
    pub fn len(&self) -> usize {
        self.inner.count.load(Ordering::Acquire)
    }

    /// Offer `event` to waiters. Clones it only when at least one predicate
    /// matches. Returns how many waiters received it.
    pub fn process(&self, event: &Event) -> usize {
        if self.inner.count.load(Ordering::Acquire) == 0 {
            return 0;
        }
        let mut guard = self.inner.waiters.lock();
        if guard.is_empty() {
            self.inner.count.store(0, Ordering::Release);
            return 0;
        }
        let mut sent = 0;
        let mut i = 0;
        while i < guard.len() {
            if guard[i].sink.is_closed() {
                guard.swap_remove(i);
                continue;
            }
            if !(guard[i].pred)(event) {
                i += 1;
                continue;
            }
            match &guard[i].sink {
                Sink::Once(_) => {
                    let waiter = guard.swap_remove(i);
                    if let Sink::Once(tx) = waiter.sink
                        && tx.send(event.clone()).is_ok()
                    {
                        sent += 1;
                    }
                }
                Sink::Stream(tx) => match tx.try_send(event.clone()) {
                    Ok(()) => {
                        sent += 1;
                        i += 1;
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        tracing::warn!("standby stream is full; event dropped");
                        i += 1;
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        guard.swap_remove(i);
                    }
                },
            }
        }
        self.inner.count.store(guard.len(), Ordering::Release);
        sent
    }

    /// Next event matching `pred`, or `None` after `timeout`.
    pub fn wait_for<F>(
        &self,
        timeout: Duration,
        pred: F,
    ) -> impl Future<Output = Option<Event>> + Send + 'static
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.inner.waiters.lock();
        guard.push(Waiter {
            id,
            pred: Arc::new(pred),
            sink: Sink::Once(tx),
        });
        self.inner.count.store(guard.len(), Ordering::Release);
        drop(guard);
        let drop_guard = WaiterGuard {
            inner: Arc::clone(&self.inner),
            id,
        };
        async move {
            let _guard = drop_guard;
            match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(event)) => Some(event),
                _ => None,
            }
        }
    }

    /// Stream of matching events until the receiver is dropped. Buffers up
    /// to [`STANDBY_STREAM_CAPACITY`] events; read them promptly.
    pub fn wait_for_stream<F>(&self, pred: F) -> mpsc::Receiver<Event>
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel(STANDBY_STREAM_CAPACITY);
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.inner.waiters.lock();
        guard.push(Waiter {
            id,
            pred: Arc::new(pred),
            sink: Sink::Stream(tx),
        });
        self.inner.count.store(guard.len(), Ordering::Release);
        drop(guard);
        rx
    }
}

impl std::fmt::Debug for Standby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Standby")
            .field("waiters", &self.inner.count.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn resumed() -> Event {
        Event::Resumed
    }

    #[tokio::test]
    async fn wait_for_matches_and_times_out() {
        let standby = Standby::new();
        let fut = standby.wait_for(Duration::from_millis(50), |e| matches!(e, Event::Resumed));
        assert_eq!(standby.process(&resumed()), 1);
        assert!(fut.await.is_some());

        let fut = standby.wait_for(Duration::from_millis(10), |_| true);
        assert!(fut.await.is_none());
        // Expired waiter is dropped; next process removes nothing harmful.
        assert_eq!(standby.process(&resumed()), 0);
    }

    #[tokio::test]
    async fn expired_waiters_are_removed_even_without_a_match() {
        let standby = Standby::new();
        for _ in 0..10 {
            let fut = standby.wait_for(Duration::from_millis(1), |_| false);
            assert!(fut.await.is_none());
        }
        let stream = standby.wait_for_stream(|_| false);
        drop(stream);
        assert!(!standby.is_empty());
        // One unrelated event clears all dead waiters.
        assert_eq!(standby.process(&resumed()), 0);
        assert!(standby.is_empty());
    }

    #[tokio::test]
    async fn stream_receives_until_dropped() {
        let standby = Standby::new();
        let mut stream = standby.wait_for_stream(|e| matches!(e, Event::Resumed));
        assert_eq!(standby.process(&resumed()), 1);
        assert_eq!(standby.process(&resumed()), 1);
        assert!(stream.recv().await.is_some());
        assert!(stream.recv().await.is_some());
        drop(stream);
        assert_eq!(standby.process(&resumed()), 0);
        assert!(standby.is_empty());
    }

    #[tokio::test]
    async fn no_clone_when_idle() {
        let standby = Standby::new();
        assert!(standby.is_empty());
        assert_eq!(standby.process(&resumed()), 0);
    }

    #[tokio::test]
    async fn dropped_wait_for_removes_waiter_immediately() {
        let standby = Standby::new();
        let fut = standby.wait_for(Duration::from_secs(60), |_| true);
        assert!(!standby.is_empty());
        drop(fut);
        assert!(standby.is_empty());
    }
}
