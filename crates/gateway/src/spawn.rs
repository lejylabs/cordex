//! Where shard tasks run (PLAN.md §16.4).
//!
//! The library never spawns on a runtime the caller did not choose. A shard's
//! long-running task is handed to a [`Spawner`]; the default
//! [`CurrentRuntime`] uses the runtime that calls `build()` / `start()`.
//! [`RuntimeHandles`] distributes shards over several runtimes, e.g. one
//! `current_thread` runtime per core (thread-per-core).

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use tokio::runtime::Handle;

use crate::error::GatewayError;
use crate::types::ShardId;

/// A boxed task as handed to a [`Spawner`].
pub type ShardTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Runs a shard's background tasks.
///
/// All tasks of one shard are handed over with the same [`ShardId`], so an
/// implementation can keep a shard on one runtime. Work a running task starts
/// itself (one task per connection) stays on that task's runtime.
pub trait Spawner: Send + Sync + 'static {
    /// Start `task` for `shard`. Fails when no runtime is available.
    fn spawn(&self, shard: ShardId, task: ShardTask) -> Result<(), GatewayError>;
}

/// Spawn on the Tokio runtime of the calling thread (default).
#[derive(Clone, Copy, Debug, Default)]
pub struct CurrentRuntime;

impl Spawner for CurrentRuntime {
    fn spawn(&self, _shard: ShardId, task: ShardTask) -> Result<(), GatewayError> {
        let handle = Handle::try_current().map_err(|_| GatewayError::NoRuntime)?;
        drop(handle.spawn(task));
        Ok(())
    }
}

/// Spread shards over a fixed set of runtimes: shard `n` runs on
/// `handles[n % handles.len()]`.
///
/// Each runtime must be driven (e.g. a thread blocked in `block_on`) for its
/// shards to make progress.
#[derive(Clone)]
pub struct RuntimeHandles {
    handles: Box<[Handle]>,
}

impl RuntimeHandles {
    /// Use these runtime handles. Fails when the list is empty.
    pub fn new(handles: impl IntoIterator<Item = Handle>) -> Result<Self, GatewayError> {
        let handles: Box<[Handle]> = handles.into_iter().collect();
        if handles.is_empty() {
            return Err(GatewayError::Validation(
                "RuntimeHandles needs at least one runtime handle".into(),
            ));
        }
        Ok(Self { handles })
    }

    /// Number of runtimes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Always `false`: construction rejects an empty list.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

impl fmt::Debug for RuntimeHandles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeHandles")
            .field("runtimes", &self.handles.len())
            .finish()
    }
}

impl Spawner for RuntimeHandles {
    fn spawn(&self, shard: ShardId, task: ShardTask) -> Result<(), GatewayError> {
        let len = self.handles.len();
        let index = usize::try_from(shard.number()).unwrap_or(usize::MAX) % len.max(1);
        let handle = self.handles.get(index).ok_or(GatewayError::NoRuntime)?;
        drop(handle.spawn(task));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn current_runtime_without_runtime_is_an_error() {
        let err = CurrentRuntime
            .spawn(ShardId::one(), Box::pin(async {}))
            .unwrap_err();
        assert!(matches!(err, GatewayError::NoRuntime), "{err}");
    }

    #[test]
    fn empty_runtime_list_is_rejected() {
        assert!(RuntimeHandles::new(Vec::new()).is_err());
    }

    #[test]
    fn runtime_handles_place_shard_by_number() {
        let rt_a = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let rt_b = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let spawner = RuntimeHandles::new([rt_a.handle().clone(), rt_b.handle().clone()]).unwrap();
        assert_eq!(spawner.len(), 2);

        let ran_a = Arc::new(AtomicUsize::new(0));
        let ran_b = Arc::new(AtomicUsize::new(0));
        for number in 0..4 {
            let target = if number % 2 == 0 { &ran_a } else { &ran_b };
            let counter = Arc::clone(target);
            spawner
                .spawn(
                    ShardId::new(number, 4).unwrap(),
                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }),
                )
                .unwrap();
        }
        // Drive only runtime A: its two shards run, B's stay queued.
        rt_a.block_on(tokio::task::yield_now());
        assert_eq!(ran_a.load(Ordering::SeqCst), 2);
        assert_eq!(ran_b.load(Ordering::SeqCst), 0);
        rt_b.block_on(tokio::task::yield_now());
        assert_eq!(ran_b.load(Ordering::SeqCst), 2);
    }
}
