use super::*;

fn noop<C>() -> Handler<C> {
    boxed(|_ctx: C| async { Ok(()) })
}

async fn test_context(bot: Bot) -> Context {
    let gateway_bot: cordex_model::gateway::GatewayBot = serde_json::from_str(
        r#"{"url":"wss://127.0.0.1:1","shards":1,"session_start_limit":{"total":1000,"remaining":1000,"reset_after":0,"max_concurrency":1}}"#,
    )
    .unwrap();
    let manager = ShardManager::builder("t", Intents::empty())
        .shards(0..1, 1)
        .gateway_bot(gateway_bot)
        .build()
        .await
        .unwrap();
    let http = Client::builder("t").build().unwrap();
    let shared = Arc::new(Shared {
        http,
        manager: Arc::new(std::sync::Mutex::new(Arc::new(manager))),
        #[cfg(feature = "cache")]
        cache: None,
        #[cfg(feature = "voice")]
        voice: None,
        me: OnceLock::new(),
        handlers: bot.handlers,
        data: bot.data,
        error_message: bot.error_message,
        auto_defer: bot.auto_defer,
        standby: Standby::new(),
    });
    Context {
        shared,
        shard: ShardId::new(0, 1).unwrap(),
    }
}

#[test]
fn lookup_prefers_exact_id_then_prefix() {
    let mut map: HashMap<Box<str>, Handler<()>> = HashMap::new();
    map.insert("vote".into(), noop());
    map.insert("vote:special".into(), noop());

    let exact = lookup(&map, "vote:special").map(Arc::as_ptr);
    assert_eq!(exact, map.get("vote:special").map(Arc::as_ptr));
    let prefix = lookup(&map, "vote:42").map(Arc::as_ptr);
    assert_eq!(prefix, map.get("vote").map(Arc::as_ptr));
    assert!(lookup(&map, "other").is_none());
    assert!(lookup(&map, "other:1").is_none());
}

#[test]
fn intents_follow_handlers() {
    let bot = Bot::new("t");
    assert_eq!(bot.intents, Intents::empty());

    let bot = Bot::new("t").on_message(|_ctx: MessageContext| async { Ok(()) });
    assert_eq!(
        bot.intents,
        Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES
    );
    assert!(!bot.intents.contains(Intents::MESSAGE_CONTENT));

    let bot = bot.message_content().intents(Intents::GUILD_MEMBERS);
    assert!(
        bot.intents
            .contains(Intents::MESSAGE_CONTENT | Intents::GUILD_MEMBERS)
    );
}

#[test]
fn same_command_name_replaces_the_old_one() {
    let bot = Bot::new("t")
        .command(
            Command::slash("ping", "old"),
            |_ctx: CommandContext| async { Ok(()) },
        )
        .command(
            Command::slash("ping", "new"),
            |_ctx: CommandContext| async { Ok(()) },
        )
        .command(Command::user("Info"), |_ctx: CommandContext| async {
            Ok(())
        });
    assert_eq!(bot.commands.len(), 2);
    assert_eq!(bot.handlers.commands.len(), 2);
    assert_eq!(bot.commands[0].name(), "ping");
}

#[test]
fn defaults() {
    let bot = Bot::new("t").shards(0);
    assert_eq!(bot.shards, Some(1));
    assert!(bot.auto_defer);
    assert_eq!(bot.error_message.as_deref(), Some(DEFAULT_ERROR_MESSAGE));
    let bot = bot.error_message(None).auto_defer(false);
    assert!(bot.error_message.is_none());
    assert!(!bot.auto_defer);
}

#[tokio::test]
async fn invalid_command_fails_before_connecting() {
    let err = Bot::new("t")
        .command(Command::slash("Ping", "x"), |_ctx: CommandContext| async {
            Ok(())
        })
        .run()
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Interaction(_)), "{err}");
    assert!(err.to_string().contains("must be lowercase"), "{err}");
}

#[tokio::test]
async fn handler_errors_and_panics_are_contained() {
    let ctx = test_context(Bot::new("t")).await;
    assert!(guarded("t", &ctx, Box::pin(async { Ok(()) })).await);
    let failing: BoxFuture =
        Box::pin(async { Err(cordex_interactions::InteractionError::NotResponded.into()) });
    assert!(!guarded("t", &ctx, failing).await);
    let panicking: BoxFuture = Box::pin(async { panic!("boom") });
    assert!(!guarded("t", &ctx, panicking).await);
}

#[tokio::test]
async fn bot_data_is_typed_and_retrievable() {
    #[derive(Debug, PartialEq)]
    struct Config {
        prefix: &'static str,
    }

    let bot = Bot::new("t")
        .data(Config { prefix: "!" })
        .data(42_u64)
        .data(99_u64);
    let ctx = test_context(bot).await;
    assert_eq!(ctx.data::<Config>(), Some(&Config { prefix: "!" }));
    assert_eq!(ctx.data::<u64>(), Some(&99_u64));
    assert_eq!(ctx.data::<String>(), None);
}

#[tokio::test]
async fn on_error_hook_receives_handler_errors() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen_hook = Arc::clone(&seen);
    let bot = Bot::new("t").data("tag").on_error(move |ctx, err| {
        let seen = Arc::clone(&seen_hook);
        async move {
            let tag = ctx.data::<&'static str>().copied().unwrap_or("");
            seen.lock().unwrap().push(format!("{tag}:{err}"));
        }
    });
    let ctx = test_context(bot).await;

    assert!(guarded("t", &ctx, Box::pin(async { Ok(()) })).await);
    assert!(seen.lock().unwrap().is_empty());

    let failing: BoxFuture =
        Box::pin(async { Err(cordex_interactions::InteractionError::NotResponded.into()) });
    assert!(!guarded("t", &ctx, failing).await);
    let entries = seen.lock().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].starts_with("tag:"), "{}", entries[0]);
}

#[test]
fn before_command_hook_is_registered() {
    let bot = Bot::new("t").before_command(|_ctx: CommandContext| async { Ok(true) });
    assert!(bot.handlers.before_command.is_some());
}

#[test]
fn only_handled_events_are_parsed() {
    let none = Bot::new("t").event_types();
    assert!(none.is_empty());

    let commands = Bot::new("t")
        .command(Command::slash("ping", "x"), |_ctx: CommandContext| async {
            Ok(())
        })
        .event_types();
    assert!(commands.wants("INTERACTION_CREATE"));
    assert!(!commands.wants("MESSAGE_CREATE"));
    assert!(!commands.wants("GUILD_CREATE"));
    assert!(!commands.wants("PRESENCE_UPDATE"));

    let messages = Bot::new("t")
        .on_message(|_ctx: MessageContext| async { Ok(()) })
        .event_types();
    assert!(messages.wants("MESSAGE_CREATE"));
    assert!(!messages.wants("INTERACTION_CREATE"));

    let all = Bot::new("t")
        .on_event(|_ctx: EventContext| async { Ok(()) })
        .event_types();
    assert_eq!(all, EventTypeFlags::all());
}

#[cfg(feature = "cache")]
#[test]
fn cache_adds_its_events_and_guilds_intent() {
    use cordex_cache::{CacheConfig, ResourceType};

    let empty = Bot::new("t").cache(CacheConfig::new());
    assert_eq!(empty.intents, Intents::empty());
    assert!(empty.event_types().is_empty());

    let bot = Bot::new("t").cache(CacheConfig::new().resources(ResourceType::CHANNELS));
    assert!(bot.intents.contains(Intents::GUILDS));
    let flags = bot.event_types();
    assert!(flags.wants("GUILD_CREATE"));
    assert!(flags.wants("CHANNEL_CREATE"));
    assert!(!flags.wants("MESSAGE_CREATE"));
}

#[test]
fn reshard_only_with_more_shards_and_enough_identifies() {
    assert!(!needs_reshard(4, 4, 1000));
    assert!(!needs_reshard(4, 2, 1000));
    assert!(needs_reshard(4, 8, 1000));
    // Not enough identifies left for the new shard set: wait.
    assert!(!needs_reshard(4, 8, 7));
    assert!(needs_reshard(4, 8, 8));
}

#[test]
fn resharding_defaults() {
    let bot = Bot::new("t");
    assert_eq!(bot.reshard_interval, Some(DEFAULT_RESHARD_INTERVAL));
    let bot = bot.reshard_interval(None);
    assert_eq!(bot.reshard_interval, None);
}
