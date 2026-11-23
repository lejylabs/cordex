use super::*;

fn noop<C>() -> Handler<C> {
    boxed(|_ctx: C| async { Ok(()) })
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
    assert!(guarded("t", Box::pin(async { Ok(()) })).await);
    let failing: BoxFuture =
        Box::pin(async { Err(cordex_interactions::InteractionError::NotResponded.into()) });
    assert!(!guarded("t", failing).await);
    let panicking: BoxFuture = Box::pin(async { panic!("boom") });
    assert!(!guarded("t", panicking).await);
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
