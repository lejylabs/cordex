//! Lifecycle-bound periodic and background tasks for [`Bot`](super::Bot).

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use super::{BoxFuture, Context, Handler, Shared};
use crate::Result;

/// Cancellation token passed to [`Bot::background`](super::Bot::background) tasks.
///
/// Triggered automatically when [`Bot::run_until_ctrl_c`](super::Bot::run_until_ctrl_c)
/// or [`Bot::run_until`](super::Bot::run_until) begins shutting down.
#[derive(Clone, Debug)]
pub struct ShutdownToken {
    rx: watch::Receiver<bool>,
}

impl ShutdownToken {
    /// Whether shutdown has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    /// Wait until shutdown is requested.
    pub async fn cancelled(&self) {
        let mut rx = self.rx.clone();
        if *rx.borrow() {
            return;
        }
        let _ = rx.wait_for(|&stopped| stopped).await;
    }
}

/// Controller for cancelling and joining spawned scheduler tasks.
pub struct SchedulerHandle {
    tx: watch::Sender<bool>,
    handles: Vec<JoinHandle<()>>,
}

impl SchedulerHandle {
    /// Signal all scheduled and background tasks to stop and wait for them
    /// (bounded by `timeout`).
    pub async fn shutdown(self, timeout: Duration) {
        let _ = self.tx.send(true);
        let join_all = async {
            for handle in self.handles {
                let _ = handle.await;
            }
        };
        let _ = tokio::time::timeout(timeout, join_all).await;
    }
}

pub(super) type BackgroundHandler = Arc<dyn Fn(Context, ShutdownToken) -> BoxFuture + Send + Sync>;

pub(super) enum ScheduledTask {
    Periodic {
        interval: Duration,
        immediate: bool,
        handler: Handler<Context>,
    },
    Background {
        handler: BackgroundHandler,
    },
}

/// Spawn all registered scheduled/background tasks and return a [`SchedulerHandle`].
pub(super) fn spawn_all(shared: &Arc<Shared>, tasks: &[ScheduledTask]) -> SchedulerHandle {
    let (tx, rx) = watch::channel(false);
    let token = ShutdownToken { rx };
    let mut join_handles = Vec::with_capacity(tasks.len());

    for task in tasks {
        let ctx = Context {
            shared: Arc::clone(shared),
            shard: cordex_gateway::ShardId::one(),
        };
        let token = token.clone();
        match task {
            ScheduledTask::Periodic {
                interval,
                immediate,
                handler,
            } => {
                let interval = *interval;
                let immediate = *immediate;
                let handler = Arc::clone(handler);
                join_handles.push(tokio::spawn(async move {
                    run_periodic(ctx, token, interval, immediate, handler).await;
                }));
            }
            ScheduledTask::Background { handler } => {
                let handler = Arc::clone(handler);
                join_handles.push(tokio::spawn(async move {
                    run_guarded("background", ctx.clone(), handler(ctx, token)).await;
                }));
            }
        }
    }

    SchedulerHandle {
        tx,
        handles: join_handles,
    }
}

async fn run_periodic(
    ctx: Context,
    token: ShutdownToken,
    interval: Duration,
    immediate: bool,
    handler: Handler<Context>,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    if !immediate {
        // Consume the immediate first tick so the first run happens after `interval`.
        ticker.tick().await;
    }

    loop {
        tokio::select! {
            () = token.cancelled() => break,
            _ = ticker.tick() => {
                run_guarded("scheduler", ctx.clone(), handler(ctx.clone())).await;
            }
        }
    }
}

async fn run_guarded(
    kind: &'static str,
    ctx: Context,
    fut: Pin<Box<dyn Future<Output = Result<()>> + Send>>,
) {
    let result = CatchUnwind(AssertUnwindSafe(fut)).await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            tracing::error!(kind, %err, "scheduled task returned an error");
            if let Some(on_error) = &ctx.shared.handlers.on_error {
                on_error(ctx.clone(), err).await;
            }
        }
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            tracing::error!(kind, panic = msg, "scheduled task panicked");
        }
    }
}

struct CatchUnwind<F>(AssertUnwindSafe<F>);

impl<F: Future + Unpin> Future for CatchUnwind<F> {
    type Output = std::thread::Result<F::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::panic::catch_unwind(AssertUnwindSafe(|| Pin::new(&mut self.0.0).poll(cx)))
            .map_or_else(|err| std::task::Poll::Ready(Err(err)), |poll| poll.map(Ok))
    }
}
