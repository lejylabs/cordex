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
        cooldown_message: bot.cooldown_message,
        cooldown_tracker: Cooldowns::new(),
        auto_defer: bot.auto_defer,
        standby: Standby::new(),
        has_message_content_intent: bot.intents.contains(Intents::MESSAGE_CONTENT),
        warned_empty_content: AtomicBool::new(false),
    });
    Context {
        shared,
        shard: ShardId::new(0, 1).unwrap(),
    }
}

#[test]
fn lookup_prefers_exact_id_then_colon_then_pattern_then_prefix() {
    let mut map: HashMap<Box<str>, Handler<()>> = HashMap::new();
    map.insert("vote".into(), noop());
    map.insert("vote:special".into(), noop());

    let patterns = vec![
        PatternRoute::parse("ticket:{action}:{id}", noop()),
        PatternRoute::parse("ticket:close:{id}", noop()),
    ];
    let prefixes = vec![
        PrefixRoute {
            prefix: "btn_".into(),
            handler: noop(),
        },
        PrefixRoute {
            prefix: "btn_admin_".into(),
            handler: noop(),
        },
    ];

    // Exact match wins over `:` prefix.
    let exact = lookup_route(&map, &patterns, &prefixes, "vote:special").unwrap();
    assert_eq!(
        Arc::as_ptr(&exact.handler),
        Arc::as_ptr(&map["vote:special"])
    );
    assert_eq!(&*exact.matched_tail, "");

    // `:` prefix match exposes tail.
    let colon = lookup_route(&map, &patterns, &prefixes, "vote:42").unwrap();
    assert_eq!(Arc::as_ptr(&colon.handler), Arc::as_ptr(&map["vote"]));
    assert_eq!(&*colon.matched_tail, "42");

    // More specific pattern (more literal segments) wins and captures params.
    let specific = lookup_route(&map, &patterns, &prefixes, "ticket:close:12345").unwrap();
    assert_eq!(
        Arc::as_ptr(&specific.handler),
        Arc::as_ptr(&patterns[1].handler)
    );
    assert_eq!(specific.params.as_ref(), &[("id".into(), "12345".into())]);

    let generic = lookup_route(&map, &patterns, &prefixes, "ticket:open:99").unwrap();
    assert_eq!(
        Arc::as_ptr(&generic.handler),
        Arc::as_ptr(&patterns[0].handler)
    );
    assert_eq!(
        generic.params.as_ref(),
        &[("action".into(), "open".into()), ("id".into(), "99".into())]
    );

    // Longest string prefix wins and exposes tail.
    let admin = lookup_route(&map, &patterns, &prefixes, "btn_admin_ban_7").unwrap();
    assert_eq!(
        Arc::as_ptr(&admin.handler),
        Arc::as_ptr(&prefixes[1].handler)
    );
    assert_eq!(&*admin.matched_tail, "ban_7");

    let general = lookup_route(&map, &patterns, &prefixes, "btn_ok").unwrap();
    assert_eq!(
        Arc::as_ptr(&general.handler),
        Arc::as_ptr(&prefixes[0].handler)
    );
    assert_eq!(&*general.matched_tail, "ok");

    assert!(lookup_route(&map, &patterns, &prefixes, "other").is_none());
    assert!(lookup_route(&map, &patterns, &prefixes, "other:1").is_none());
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
    assert_eq!(bot.shards, Some(BotShards::Range(0..1, 1)));
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
    let bot = Bot::new("t")
        .before_command(|_ctx: CommandContext| async { Ok(true) })
        .before_command(|_ctx: CommandContext| async { Ok(true) });
    assert_eq!(bot.handlers.before_command.len(), 2);
}

#[test]
fn module_mount_merges_commands_intents_and_routes() {
    let child = BotModule::new()
        .slash("ban", "Ban a user", |_ctx: CommandContext| async { Ok(()) })
        .on_button_pattern("ticket:{action}:{id}", |_ctx: ComponentContext| async {
            Ok(())
        });

    let moderation = BotModule::new()
        .before_command(|_ctx: CommandContext| async { Ok(true) })
        .command_with_cooldown(
            Command::slash("warn", "Warn a user"),
            Duration::from_secs(10),
            CooldownScope::User,
            |_ctx: CommandContext| async { Ok(()) },
        )
        .on_message(|_ctx: MessageContext| async { Ok(()) })
        .mount(child);

    let bot = Bot::new("t")
        .slash("ping", "Ping", |_ctx: CommandContext| async { Ok(()) })
        .mount(moderation);

    assert_eq!(bot.commands.len(), 3);
    assert!(bot.handlers.commands.contains_key("ping"));
    assert!(bot.handlers.commands.contains_key("warn"));
    assert!(bot.handlers.commands.contains_key("ban"));
    assert!(bot.handlers.command_cooldowns.contains_key("warn"));
    assert_eq!(bot.handlers.component_patterns.len(), 1);
    assert!(
        bot.intents
            .contains(Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES)
    );
    // Module-scoped `before_command` wraps module commands without polluting root guards.
    assert!(bot.handlers.before_command.is_empty());
    assert!(bot.event_types().wants("INTERACTION_CREATE"));
    assert!(bot.event_types().wants("MESSAGE_CREATE"));
}

#[test]
fn from_env_var_reports_missing_variable() {
    let err = Bot::from_env_var("CORDEX_TEST_NONEXISTENT_TOKEN_987654").unwrap_err();
    assert!(matches!(err, Error::MissingEnv(_)), "{err}");
    assert!(
        err.to_string()
            .contains("CORDEX_TEST_NONEXISTENT_TOKEN_987654"),
        "{err}"
    );
}

#[test]
fn pattern_and_prefix_routes_enable_interaction_events() {
    let bot = Bot::new("t").on_button_prefix("ticket_", |_ctx: ComponentContext| async { Ok(()) });
    assert!(bot.event_types().wants("INTERACTION_CREATE"));

    let bot = Bot::new("t").on_modal_pattern("report:{id}", |_ctx: ModalContext| async { Ok(()) });
    assert!(bot.event_types().wants("INTERACTION_CREATE"));
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

#[test]
fn guard_errors_format_cleanly() {
    use std::num::NonZeroU64;

    let role_id = cordex_model::RoleId::from(NonZeroU64::MIN);
    let sku_id = cordex_model::id::Id::<cordex_model::id::marker::SkuMarker>::from(NonZeroU64::MIN);

    assert!(Error::GuildOnly.to_string().contains("server"));
    assert!(Error::NsfwOnly.to_string().contains("NSFW"));
    assert!(Error::MissingRole(role_id).to_string().contains("role"));
    assert!(
        Error::MissingEntitlement(sku_id)
            .to_string()
            .contains("entitlement")
    );
    assert!(Error::OwnerOnly.to_string().contains("owner"));
}

#[test]
fn collector_end_reason_variants_are_distinct() {
    let reasons = [
        CollectorEndReason::Timeout,
        CollectorEndReason::Idle,
        CollectorEndReason::Limit,
        CollectorEndReason::Stopped,
        CollectorEndReason::Closed,
    ];
    for (i, a) in reasons.iter().enumerate() {
        for (j, b) in reasons.iter().enumerate() {
            assert_eq!(a == b, i == j);
        }
    }
}
