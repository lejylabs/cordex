//! Minimal live test bot.
//!
//! Reads `DISCORD_TOKEN` (and optionally `SHARDS` = number, unset/`auto` =
//! automatic, `PREFIX`) from the environment or from a `.env` file in the
//! current directory and connects to the gateway.
//!
//! Text commands (need the privileged `MESSAGE_CONTENT` intent):
//!
//! - `!ping`: replies with the shard's heartbeat latency.
//! - `!status`: state, latency, guilds and reconnects of every shard.
//! - `!reconnect`: restarts the shard that received the message.
//! - `!cache`: entry counts of the cache (guilds, channels, roles,
//!   members, users and the last 20 messages per channel are on).
//!
//! Slash commands, registered per guild on READY (show up instantly):
//!
//! - `/ping`: latency, as an interaction response.
//! - `/echo text [private]`: repeats the text, optionally ephemeral.
//! - `/slow`: auto-defer after 2.5 s, answers after 5 s (defer → edit).
//! - `/color query`: autocomplete suggestions.
//! - `/whois user`: user option + follow-up message.
//! - `User Info`: user context-menu command.
//!
//! Run with `cargo run -p cordex --example testbot --features cache`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use cordex::gateway::{ManagerEvent, ShardManager, ShardManagerBuilder};
use cordex::http::Client;
use cordex::interactions::handle::DEFAULT_AUTO_DEFER;
use cordex::model::gateway::event::Ready;
use cordex::model::interaction::{Interaction, InteractionData, InteractionType};
use cordex::prelude::*;

type AnyResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const COLORS: [&str; 8] = [
    "red", "orange", "yellow", "green", "blue", "indigo", "violet", "black",
];

#[tokio::main]
async fn main() -> AnyResult {
    // Library logs (warnings by default); override with RUST_LOG, e.g.
    // `RUST_LOG=cordex_gateway=debug`.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();
    let config = Config::load(".env");
    let token = config
        .var("DISCORD_TOKEN")
        .ok_or("DISCORD_TOKEN is not set (put it into .env)")?;
    let shards = config.var("SHARDS").unwrap_or_else(|| "auto".to_owned());
    let prefix = config.var("PREFIX").unwrap_or_else(|| "!".to_owned());

    let http = Client::new(&token)?;
    let intents = Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT;

    let builder = ShardManager::builder(&token, intents).http(http.clone());
    let mut manager = apply_shards(builder, &shards)?.build().await?;
    let mut events = manager.events();
    let manager = Arc::new(manager);
    // Cache with a few resources on, to try the switches live (`!cache`).
    let cache = Arc::new(InMemoryCache::new(
        CacheConfig::new()
            .resources(
                ResourceType::GUILDS
                    | ResourceType::CHANNELS
                    | ResourceType::ROLES
                    | ResourceType::MEMBERS
                    | ResourceType::USERS
                    | ResourceType::MESSAGES,
            )
            .parts(CacheParts::MEMBER_ROLES | CacheParts::MESSAGE_CONTENT)
            .message_cache_size(20),
    ));
    manager.start().await?;
    println!(
        "started {} shard(s) (SHARDS={shards}, Discord recommends {}), prefix {prefix:?}; Ctrl+C to quit",
        manager.total_shards(),
        manager.recommended_shards()
    );

    while let Some(item) = events.recv().await {
        match item {
            ManagerEvent::Dispatch { shard, event } => {
                cache.update(&event);
                match event {
                    Event::Ready(ready) => {
                        println!(
                            "[shard {}] READY as {} ({} guilds)",
                            shard.number(),
                            ready.user.username,
                            ready.guilds.len()
                        );
                        tokio::spawn(register_commands(http.clone(), ready));
                    }
                    Event::Resumed => println!("[shard {}] RESUMED", shard.number()),
                    Event::MessageDelete(d) => log_delete(shard.number(), &d),
                    Event::InteractionCreate(interaction) => {
                        let (http, manager) = (http.clone(), Arc::clone(&manager));
                        let number = shard.number();
                        tokio::spawn(async move {
                            if let Err(e) =
                                on_interaction(&http, &manager, number, &interaction).await
                            {
                                eprintln!("[shard {number}] interaction failed: {e}");
                            }
                        });
                    }
                    Event::MessageCreate(msg) if !msg.author.bot => {
                        if let Some(cmd) = msg.content.strip_prefix(prefix.as_str()) {
                            on_text_command(
                                &http,
                                &manager,
                                &cache,
                                shard.number(),
                                &msg,
                                cmd.trim(),
                            );
                        }
                    }
                    _ => {}
                }
            }
            ManagerEvent::StateChanged { shard, old, new } => {
                println!("[shard {}] {old:?} -> {new:?}", shard.number());
            }
            ManagerEvent::Error { shard, error } => {
                eprintln!("[shard {}] fatal: {error}", shard.number());
            }
            other => println!("{other:?}"),
        }
    }
    Ok(())
}

fn commands() -> Vec<Command> {
    vec![
        Command::slash("ping", "Heartbeat latency of this shard"),
        Command::slash("echo", "Repeat a text")
            .option(
                CommandOption::string("text", "What to repeat")
                    .required()
                    .max_length(500),
            )
            .option(CommandOption::boolean("private", "Only you see the answer")),
        Command::slash("slow", "Answers after 5 seconds (tests auto-defer)"),
        Command::slash("color", "Pick a color").option(
            CommandOption::string("query", "Start typing")
                .required()
                .autocomplete(),
        ),
        Command::slash("whois", "Show a user").option(
            CommandOption::user("user", "Who")
                .required()
                .name_localization("de", "nutzer"),
        ),
        Command::user("User Info"),
    ]
}

/// Guild commands update instantly (global ones can take a while).
async fn register_commands(http: Client, ready: Box<Ready>) {
    let app = ready.application.id;
    for guild in &ready.guilds {
        match http
            .bulk_overwrite_guild_commands(app, guild.id, commands())
            .await
        {
            Ok(list) => println!("registered {} commands in guild {}", list.len(), guild.id),
            Err(e) => eprintln!("registering commands in guild {} failed: {e}", guild.id),
        }
    }
}

async fn on_interaction(
    http: &Client,
    manager: &ShardManager,
    shard: u32,
    interaction: &Interaction,
) -> AnyResult {
    let handle = InteractionHandle::new(http.clone(), interaction);
    let Some(InteractionData::Command(data)) = &interaction.data else {
        return Ok(());
    };
    let opts = CommandOptions::new(data);
    let who = interaction.author().map_or("?", |u| &*u.username);
    println!("[shard {shard}] /{} by {who}", opts.command());

    if interaction.kind == InteractionType::ApplicationCommandAutocomplete {
        let typed = opts.focused().and_then(|o| match &o.value {
            Some(cordex::model::interaction::CommandOptionValue::String(s)) => {
                Some(s.to_lowercase())
            }
            _ => None,
        });
        let typed = typed.unwrap_or_default();
        let hits = COLORS
            .iter()
            .filter(|c| c.starts_with(&typed))
            .map(|c| Choice::string(*c, *c));
        handle.autocomplete(hits).await?;
        return Ok(());
    }

    match opts.command() {
        "ping" => {
            let latency = manager.shard(shard).and_then(|s| s.latency().latest);
            handle
                .respond(format!(
                    "pong (shard {shard}, latency {}, {} ms since the event arrived)",
                    fmt_latency(latency),
                    handle.elapsed().as_millis()
                ))
                .await?;
        }
        "echo" => {
            let text = opts.string("text")?;
            let reply = Reply::new(text);
            let reply = if opts.boolean_opt("private")?.unwrap_or(false) {
                reply.ephemeral()
            } else {
                reply
            };
            handle.respond(reply).await?;
        }
        "slow" => {
            handle.auto_defer(DEFAULT_AUTO_DEFER, false);
            tokio::time::sleep(Duration::from_secs(5)).await;
            handle
                .respond("done after 5 s (deferred automatically, then edited)")
                .await?;
        }
        "color" => {
            handle
                .respond(format!("you picked {}", opts.string("query")?))
                .await?;
        }
        "whois" | "User Info" => {
            let user = if opts.command() == "whois" {
                opts.user("user")?
            } else {
                opts.target_user()?
            };
            let nick = user.member.and_then(|m| m.nick.as_deref()).unwrap_or("-");
            handle
                .respond(Reply::new(format!(
                    "{} (id {}, bot: {}, nick: {nick})",
                    user.user.username, user.user.id, user.user.bot
                )))
                .await?;
            handle
                .followup(Reply::new("this is a follow-up message").ephemeral())
                .await?;
        }
        other => handle.respond(format!("unknown command {other}")).await?,
    }
    Ok(())
}

fn on_text_command(
    http: &Client,
    manager: &Arc<ShardManager>,
    cache: &InMemoryCache,
    shard: u32,
    msg: &Message,
    cmd: &str,
) {
    let text = match cmd {
        "ping" => {
            let latency = manager.shard(shard).and_then(|s| s.latency().latest);
            format!("pong (shard {shard}, latency {})", fmt_latency(latency))
        }
        "status" => status_text(manager),
        "cache" => {
            let s = cache.stats();
            let in_channel = cache.channel_messages(msg.channel_id).len();
            format!(
                "cache: {} guilds, {} channels, {} roles, {} members, {} users, {} messages ({in_channel} in this channel)",
                s.guilds, s.channels, s.roles, s.members, s.users, s.messages
            )
        }
        "reconnect" => {
            let manager = Arc::clone(manager);
            tokio::spawn(async move {
                match manager.restart_shard(shard).await {
                    Ok(()) => println!("[shard {shard}] restarted"),
                    Err(e) => eprintln!("[shard {shard}] restart failed: {e}"),
                }
            });
            format!("restarting shard {shard}")
        }
        _ => return,
    };
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

/// Snowflake → creation time; shows how soon after sending a message
/// disappeared.
fn log_delete(shard: u32, d: &cordex::model::gateway::event::MessageDelete) {
    let created_ms = (d.id.get() >> 22) + 1_420_070_400_000;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| u64::try_from(t.as_millis()).unwrap_or(u64::MAX));
    println!(
        "[shard {shard}] MESSAGE_DELETE {} in channel {} ({} ms after it was sent)",
        d.id,
        d.channel_id,
        now_ms.saturating_sub(created_ms)
    );
}

/// Discord sends the first heartbeat after a random part of the interval
/// (up to ~41 s), so right after READY there is no sample yet.
fn fmt_latency(latency: Option<Duration>) -> String {
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
