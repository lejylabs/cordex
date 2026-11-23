//! Low level: run a fixed range of shards yourself, e.g. one process per
//! range in a multi-process deployment, and handle raw gateway events.
//!
//! ```text
//! DISCORD_TOKEN=... SHARD_FROM=0 SHARD_TO=2 SHARD_TOTAL=4 \
//!     cargo run -p cordex --example manual_sharding
//! ```

use cordex::gateway::{ManagerEvent, ShardManager};
use cordex::prelude::*;

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("DISCORD_TOKEN")?;
    let (from, to, total) = (
        env_u32("SHARD_FROM", 0),
        env_u32("SHARD_TO", 1),
        env_u32("SHARD_TOTAL", 1),
    );

    let http = Client::new(&token)?;
    let mut manager = ShardManager::builder(&token, Intents::GUILDS | Intents::GUILD_MESSAGES)
        .http(http)
        // Only parse what we use; everything else is skipped cheaply.
        .event_types(
            EventTypeFlags::READY | EventTypeFlags::GUILD_CREATE | EventTypeFlags::MESSAGE_CREATE,
        )
        .shards(from..to, total)
        .build()
        .await?;
    let mut events = manager.events();
    manager.start().await?;
    println!("running shards {from}..{to} of {total}");

    while let Some(item) = events.recv().await {
        match item {
            ManagerEvent::Dispatch { shard, event } => match event {
                Event::Ready(ready) => {
                    println!(
                        "[{}] ready with {} guilds",
                        shard.number(),
                        ready.guilds.len()
                    );
                }
                Event::GuildCreate(_) => println!("[{}] guild available", shard.number()),
                Event::MessageCreate(message) => {
                    println!(
                        "[{}] #{}: {}",
                        shard.number(),
                        message.channel_id,
                        message.content
                    );
                }
                _ => {}
            },
            ManagerEvent::Error { shard, error } => {
                eprintln!("[{}] stopped: {error}", shard.number());
            }
            _ => {}
        }
    }
    Ok(())
}
