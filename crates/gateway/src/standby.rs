//! Wait for the next matching gateway event (Twilight's `standby`).
//!
//! Registered waiters are checked in [`Standby::process`]; the event is
//! cloned only when a waiter matches. Dropped or timed-out waiters are
//! removed on the next event.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use cordex_model::gateway::Event;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

type Pred = Arc<dyn Fn(&Event) -> bool + Send + Sync>;

enum Sink {
    Once(oneshot::Sender<Event>),
    Stream(mpsc::UnboundedSender<Event>),
}

struct Waiter {
    pred: Pred,
    sink: Sink,
}

/// Shared list of event waiters. Cheap to clone (`Arc`).
#[derive(Clone, Default)]
pub struct Standby {
    inner: Arc<Mutex<Vec<Waiter>>>,
}

impl Standby {
    /// Empty waiter list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anyone is waiting.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// Offer `event` to waiters. Clones it only when at least one predicate
    /// matches. Returns how many waiters received it.
    pub fn process(&self, event: &Event) -> usize {
        let mut guard = self.inner.lock();
        if guard.is_empty() {
            return 0;
        }
        let mut sent = 0;
        let mut i = 0;
        while i < guard.len() {
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
                Sink::Stream(tx) => {
                    if tx.send(event.clone()).is_err() {
                        guard.swap_remove(i);
                    } else {
                        sent += 1;
                        i += 1;
                    }
                }
            }
        }
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
        self.inner.lock().push(Waiter {
            pred: Arc::new(pred),
            sink: Sink::Once(tx),
        });
        async move {
            match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(event)) => Some(event),
                _ => None,
            }
        }
    }

    /// Stream of matching events until the receiver is dropped.
    pub fn wait_for_stream<F>(&self, pred: F) -> mpsc::UnboundedReceiver<Event>
    where
        F: Fn(&Event) -> bool + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        self.inner.lock().push(Waiter {
            pred: Arc::new(pred),
            sink: Sink::Stream(tx),
        });
        rx
    }
}

impl std::fmt::Debug for Standby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Standby")
            .field("waiters", &self.inner.lock().len())
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
    async fn no_clone_when_idle() {
        let standby = Standby::new();
        assert!(standby.is_empty());
        assert_eq!(standby.process(&resumed()), 0);
    }
}
