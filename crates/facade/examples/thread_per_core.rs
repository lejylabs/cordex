//! Thread-per-core: pin shards onto dedicated `current_thread` Tokio runtimes
//! using [`RuntimeHandles`], one worker thread per core.
//!
//! ```text
//! DISCORD_TOKEN=... CORES=2 cargo run -p cordex --example thread_per_core
//! ```

use std::sync::{Arc, mpsc};
use std::thread;

use cordex::gateway::{ManagerEvent, RuntimeHandles, ShardManager};
use cordex::prelude::*;
use tokio::runtime::Builder;
use tokio::sync::oneshot;

fn worker_count() -> usize {
    std::env::var("CORES")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| thread::available_parallelism().map_or(2, std::num::NonZeroUsize::get))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("DISCORD_TOKEN")?;
    let cores = worker_count();

    // Start one single-threaded Tokio runtime per worker thread and keep it
    // alive until the main thread drops the shutdown senders.
    let mut handles = Vec::with_capacity(cores);
    let mut shutdowns = Vec::with_capacity(cores);
    let mut threads = Vec::with_capacity(cores);

    for core in 0..cores {
        let (handle_tx, handle_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let join = thread::Builder::new()
            .name(format!("cordex-core-{core}"))
            .spawn(move || {
                let Ok(rt) = Builder::new_current_thread().enable_all().build() else {
                    return;
                };
                let _ = handle_tx.send(rt.handle().clone());
                rt.block_on(async {
                    let _ = stop_rx.await;
                });
            })?;
        handles.push(handle_rx.recv()?);
        shutdowns.push(stop_tx);
        threads.push(join);
    }

    let spawner = Arc::new(RuntimeHandles::new(handles)?);

    // Drive the coordinator loop on a small current_thread runtime while
    // shard tasks execute on the worker threads above.
    let main_rt = Builder::new_current_thread().enable_all().build()?;
    let result: Result<(), Box<dyn std::error::Error>> = main_rt.block_on(async move {
        let http = Client::new(&token)?;
        let mut manager = ShardManager::builder(&token, Intents::GUILDS)
            .http(http)
            .spawn_with(spawner)
            .event_types(EventTypeFlags::READY | EventTypeFlags::GUILD_CREATE)
            .automatic()
            .build()
            .await?;
        let mut events = manager.events();
        manager.start().await?;
        println!("running shards across {cores} current_thread runtime(s)");

        while let Some(item) = events.recv().await {
            match item {
                ManagerEvent::Dispatch {
                    shard,
                    event: Event::Ready(ready),
                } => {
                    println!(
                        "[shard {}] ready with {} guilds",
                        shard.number(),
                        ready.guilds.len()
                    );
                }
                ManagerEvent::Error { shard, error } => {
                    eprintln!("[shard {}] error: {error}", shard.number());
                }
                _ => {}
            }
        }
        Ok(())
    });

    for stop in shutdowns {
        let _ = stop.send(());
    }
    for join in threads {
        let _ = join.join();
    }
    result
}
