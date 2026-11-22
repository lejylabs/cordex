//! Minimal live test bot (PLAN.md §17.5).
//!
//! Reads `DISCORD_TOKEN` (and optionally `SHARDS` = number, unset/`auto` = automatic, `PREFIX`) from the
//! environment or from a `.env` file in the current directory, connects to
//! the gateway and answers a few text commands:
//!
//! - `!ping`: replies with the shard's heartbeat latency.
//! - `!status`: state, latency, guilds and reconnects of every shard.
//! - `!reconnect`: restarts the shard that received the message.
//!
//! Needs the privileged `MESSAGE_CONTENT` intent enabled in the developer
//! portal. Run with `cargo run -p drs --example testbot`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use drs::gateway::{ManagerEvent, ShardManager, ShardManagerBuilder};
use drs::http::Client;
use drs::prelude::*;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(".env");
    let token = config
        .var("DISCORD_TOKEN")
        .ok_or("DISCORD_TOKEN is not set (put it into .env)")?;
    // Unset or `auto`: the count from `GET /gateway/bot`; a number runs
    // shards `0..n` of `n`.
    let shards = config.var("SHARDS").unwrap_or_else(|| "auto".to_owned());
    let prefix = config.var("PREFIX").unwrap_or_else(|| "!".to_owned());

    let http = Client::new(&token)?;
    let intents = Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT;

    let builder = ShardManager::builder(&token, intents).http(http.clone());
    let builder = apply_shards(builder, &shards)?;
    let mut manager = builder.build().await?;
    let mut events = manager.events();
    let manager = Arc::new(manager);
    manager.start().await?;
    println!(
        "started {} shard(s) (SHARDS={shards}, Discord recommends {}), prefix {prefix:?}; Ctrl+C to quit",
        manager.total_shards(),
        manager.recommended_shards()
    );

    while let Some(item) = events.recv().await {
        match item {
            ManagerEvent::Dispatch { shard, event } => match event {
                Event::Ready(ready) => println!(
                    "[shard {}] READY as {} ({} guilds)",
                    shard.number(),
                    ready.user.username,
                    ready.guilds.len()
                ),
                Event::Resumed => println!("[shard {}] RESUMED", shard.number()),
                Event::MessageCreate(msg) if !msg.author.bot => {
                    let Some(cmd) = msg.content.strip_prefix(prefix.as_str()) else {
                        continue;
                    };
                    let reply = match cmd.trim() {
                        "ping" => {
                            let latency = manager
                                .shard(shard.number())
                                .and_then(|s| s.latency().latest);
                            Some(format!(
                                "pong (shard {}, latency {})",
                                shard.number(),
                                fmt_latency(latency)
                            ))
                        }
                        "status" => Some(status_text(&manager)),
                        "reconnect" => {
                            let manager = Arc::clone(&manager);
                            let number = shard.number();
                            tokio::spawn(async move {
                                match manager.restart_shard(number).await {
                                    Ok(()) => println!("[shard {number}] restarted"),
                                    Err(e) => eprintln!("[shard {number}] restart failed: {e}"),
                                }
                            });
                            Some(format!("restarting shard {}", shard.number()))
                        }
                        _ => None,
                    };
                    if let Some(text) = reply {
                        let http = http.clone();
                        let (channel, id) = (msg.channel_id, msg.id);
                        tokio::spawn(async move {
                            let sent = match http.create_message(channel).content(text) {
                                Ok(req) => req.reply(id).send().await.map(|_| ()),
                                Err(e) => Err(e),
                            };
                            if let Err(e) = sent {
                                eprintln!("reply failed: {e}");
                            }
                        });
                    }
                }
                _ => {}
            },
            ManagerEvent::StateChanged { shard, old, new } => {
                println!("[shard {}] {old:?} -> {new:?}", shard.number());
            }
            ManagerEvent::Error { shard, error } => {
                eprintln!("[shard {}] fatal: {error}", shard.number());
            }
            ManagerEvent::RecommendedShardCount {
                running,
                recommended,
            } => {
                println!("note: running {running} shard(s), Discord recommends {recommended}");
            }
            other => println!("{other:?}"),
        }
    }
    Ok(())
}

/// Discord sends the first heartbeat after a random part of the interval
/// (up to ~41 s), so right after READY there is no sample yet.
fn fmt_latency(latency: Option<std::time::Duration>) -> String {
    latency.map_or_else(
        || "n/a (no heartbeat ACK yet)".to_owned(),
        |d| format!("{} ms", d.as_millis()),
    )
}

/// `auto` → [`ShardManagerBuilder::automatic`], `n` → shards `0..n` of `n`.
fn apply_shards(
    builder: ShardManagerBuilder,
    shards: &str,
) -> std::result::Result<ShardManagerBuilder, &'static str> {
    if shards.eq_ignore_ascii_case("auto") {
        return Ok(builder.automatic());
    }
    let n = shards
        .parse::<u32>()
        .map_err(|_| "SHARDS must be a number or `auto`")?
        .max(1);
    Ok(builder.shards(0..n, n))
}

fn status_text(manager: &ShardManager) -> String {
    let mut out = String::from("```\n");
    for s in manager.status() {
        let _ = writeln!(
            out,
            "shard {}/{}: {:?}, latency {}, guilds {}, reconnects {}",
            s.id.number(),
            s.id.total(),
            s.state,
            fmt_latency(s.latency.latest),
            s.guild_count,
            s.reconnect_count
        );
    }
    out.push_str("```");
    out
}

/// Tiny `.env` reader (`KEY=VALUE`, `#` comments, optional quotes).
/// `std::env::set_var` is `unsafe` in edition 2024, so the values stay in a
/// map; [`Config::var`] prefers real environment variables.
struct Config(HashMap<String, String>);

impl Config {
    fn load(path: &str) -> Self {
        let mut map = HashMap::new();
        if let Ok(text) = std::fs::read_to_string(path) {
            for line in text.lines().map(str::trim) {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
                    map.insert(key.trim().to_owned(), value.to_owned());
                }
            }
        }
        Self(map)
    }

    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .or_else(|| self.0.get(key).cloned())
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }
}
