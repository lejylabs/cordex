//! Bounded event pipe so heartbeats never wait on `next_event`.

use parking_lot::{Mutex, MutexGuard};
use std::collections::VecDeque;
use tokio::sync::Notify;

use cordex_model::gateway::Event;

use crate::error::GatewayError;
use crate::types::Backpressure;

pub(crate) enum Push {
    Ok,
    Full(cordex_model::gateway::Event),
    Disconnect,
}

pub(crate) struct EventPipe {
    inner: Mutex<Inner>,
    notify_recv: Notify,
    notify_space: Notify,
}

struct Inner {
    items: VecDeque<Result<Event, GatewayError>>,
    cap: usize,
    closed: bool,
    dropped: u64,
}

impl EventPipe {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                items: VecDeque::with_capacity(cap.min(64)),
                cap: cap.max(1),
                closed: false,
                dropped: 0,
            }),
            notify_recv: Notify::new(),
            notify_space: Notify::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock()
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.lock().dropped
    }

    pub(crate) fn len(&self) -> usize {
        self.lock().items.len()
    }

    pub(crate) fn has_space(&self) -> bool {
        let inner = self.lock();
        inner.items.len() < inner.cap && !inner.closed
    }

    pub(crate) async fn wait_space(&self) {
        loop {
            if self.has_space() {
                return;
            }
            self.notify_space.notified().await;
        }
    }

    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.notify_recv.notify_waiters();
        self.notify_space.notify_waiters();
    }

    pub(crate) fn push_error(&self, err: GatewayError) {
        let mut inner = self.lock();
        if inner.closed {
            return;
        }
        if inner.items.len() >= inner.cap {
            inner.items.pop_front();
            inner.dropped += 1;
        }
        inner.items.push_back(Err(err));
        inner.closed = true;
        drop(inner);
        self.notify_recv.notify_waiters();
        self.notify_space.notify_waiters();
    }

    /// Deliver a non-fatal error (e.g. a guarded panic) without closing the
    /// pipe; the shard keeps running.
    pub(crate) fn push_soft_error(&self, err: GatewayError) {
        let mut inner = self.lock();
        if inner.closed {
            return;
        }
        if inner.items.len() >= inner.cap {
            inner.items.pop_front();
            inner.dropped += 1;
        }
        inner.items.push_back(Err(err));
        drop(inner);
        self.notify_recv.notify_one();
    }

    pub(crate) fn push_event(&self, event: Event, policy: Backpressure) -> Push {
        let mut inner = self.lock();
        if inner.closed {
            return Push::Ok;
        }
        if inner.items.len() < inner.cap {
            inner.items.push_back(Ok(event));
            drop(inner);
            self.notify_recv.notify_one();
            return Push::Ok;
        }
        match policy {
            Backpressure::DropNewest => {
                inner.dropped += 1;
                Push::Ok
            }
            Backpressure::DropOldest => {
                inner.items.pop_front();
                inner.dropped += 1;
                inner.items.push_back(Ok(event));
                drop(inner);
                self.notify_recv.notify_one();
                Push::Ok
            }
            Backpressure::Disconnect => Push::Disconnect,
            Backpressure::Block => Push::Full(event),
        }
    }

    pub(crate) fn try_pop(&self) -> Option<Result<Event, GatewayError>> {
        let mut inner = self.lock();
        let item = inner.items.pop_front();
        let closed = inner.closed;
        drop(inner);
        if item.is_some() {
            self.notify_space.notify_one();
        }
        let _ = closed;
        item
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    pub(crate) async fn pop(&self) -> Option<Result<Event, GatewayError>> {
        loop {
            let notified = self.notify_recv.notified();
            if let Some(item) = self.try_pop() {
                return Some(item);
            }
            if self.is_closed() {
                return None;
            }
            notified.await;
        }
    }
}
