//! Replay, switch, limit and concurrency tests for the cache.

use std::sync::Arc;

use cordex_model::gateway::Event;
use cordex_model::id::Id;
use serde_json::value::RawValue;
use serde_json::{Value, json};

use crate::{CacheConfig, CacheParts, CacheStats, ConfigWarning, InMemoryCache, ResourceType};

const GUILD: u64 = 1;
const TEXT: u64 = 100;
const THREAD: u64 = 200;
const ALICE: u64 = 500;
const BOB: u64 = 501;
const BOT: u64 = 999;

fn fixture(name: &str) -> Value {
    let raw = match name {
        "guild" => include_str!("../../model/tests/fixtures/guild.json"),
        "user" => include_str!("../../model/tests/fixtures/user.json"),
        "member" => include_str!("../../model/tests/fixtures/guild_member.json"),
        "role" => include_str!("../../model/tests/fixtures/role.json"),
        "emoji" => include_str!("../../model/tests/fixtures/emoji.json"),
        "sticker" => include_str!("../../model/tests/fixtures/sticker.json"),
        "text" => include_str!("../../model/tests/fixtures/channel_text.json"),
        "thread" => include_str!("../../model/tests/fixtures/channel_thread.json"),
        "stage" => include_str!("../../model/tests/fixtures/stage_instance.json"),
        "sound" => include_str!("../../model/tests/fixtures/soundboard_guild.json"),
        "message" => include_str!("../../model/tests/fixtures/message.json"),
        "activity" => include_str!("../../model/tests/fixtures/activity.json"),
        other => panic!("unknown fixture {other}"),
    };
    serde_json::from_str(raw).unwrap()
}

fn ev(name: &str, data: &Value) -> Event {
    let raw = RawValue::from_string(data.to_string()).unwrap();
    Event::from_dispatch(name, Some(&raw)).unwrap()
}

fn id(n: u64) -> Value {
    Value::String(n.to_string())
}

fn user(n: u64) -> Value {
    let mut u = fixture("user");
    u["id"] = id(n);
    u
}

fn member(n: u64) -> Value {
    let mut m = fixture("member");
    m["user"] = user(n);
    m["roles"] = json!([id(10)]);
    m["nick"] = json!("nick");
    m
}

fn presence(n: u64) -> Value {
    json!({
        "user": {"id": id(n)}, "guild_id": id(GUILD), "status": "online",
        "activities": [fixture("activity")], "client_status": {"desktop": "online"}
    })
}

fn message(n: u64, channel: u64) -> Value {
    let mut m = fixture("message");
    m["id"] = id(n);
    m["channel_id"] = id(channel);
    m["guild_id"] = id(GUILD);
    m["author"] = user(ALICE);
    m["embeds"] = json!([{"title": "embed"}]);
    m["attachments"] = json!([{"id": "1", "filename": "a.png", "size": 1,
        "url": "https://cdn/a.png", "proxy_url": "https://media/a.png"}]);
    m
}

fn guild_create() -> Value {
    let mut g = fixture("guild");
    g["id"] = id(GUILD);
    let mut role = fixture("role");
    role["id"] = id(10);
    g["roles"] = json!([role]);
    let mut emoji = fixture("emoji");
    emoji["id"] = id(20);
    g["emojis"] = json!([emoji]);
    let mut sticker = fixture("sticker");
    sticker["id"] = id(30);
    sticker["guild_id"] = id(GUILD);
    g["stickers"] = json!([sticker]);
    g["joined_at"] = json!("2024-01-01T00:00:00.000000+00:00");
    g["large"] = json!(false);
    g["member_count"] = json!(2);
    let mut text = fixture("text");
    text["id"] = id(TEXT);
    text.as_object_mut().unwrap().remove("guild_id");
    text["permission_overwrites"] =
        json!([{"id": id(GUILD), "type": 0, "allow": "0", "deny": "2048"}]);
    g["channels"] = json!([text]);
    let mut thread = fixture("thread");
    thread["id"] = id(THREAD);
    thread["parent_id"] = id(TEXT);
    thread.as_object_mut().unwrap().remove("guild_id");
    g["threads"] = json!([thread]);
    g["members"] = json!([member(ALICE), member(BOB)]);
    g["voice_states"] = json!([{
        "user_id": id(ALICE), "channel_id": id(TEXT), "session_id": "s",
        "deaf": false, "mute": false, "self_deaf": false, "self_mute": false,
        "self_video": false, "suppress": false, "member": member(ALICE)
    }]);
    g["presences"] = json!([presence(ALICE)]);
    let mut stage = fixture("stage");
    stage["id"] = id(300);
    stage["guild_id"] = id(GUILD);
    g["stage_instances"] = json!([stage]);
    g["guild_scheduled_events"] = json!([{
        "id": id(350), "guild_id": id(GUILD), "name": "event",
        "scheduled_start_time": "2030-01-01T00:00:00.000000+00:00",
        "privacy_level": 2, "status": 1, "entity_type": 2, "channel_id": id(TEXT)
    }]);
    let mut sound = fixture("sound");
    sound["sound_id"] = id(400);
    g["soundboard_sounds"] = json!([sound]);
    g
}

fn ready() -> Value {
    json!({
        "v": 10, "user": user(BOT), "guilds": [{"id": id(GUILD), "unavailable": true}],
        "session_id": "s", "resume_gateway_url": "wss://r", "application": {"id": "7", "flags": 0}
    })
}

/// `READY`, `GUILD_CREATE` and two messages: touches every resource.
fn stream() -> Vec<Event> {
    vec![
        ev("READY", &ready()),
        ev("GUILD_CREATE", &guild_create()),
        ev("MESSAGE_CREATE", &message(1, TEXT)),
        ev("MESSAGE_CREATE", &message(2, TEXT)),
    ]
}

fn replay(config: CacheConfig) -> InMemoryCache {
    let cache = InMemoryCache::new(config);
    for e in stream() {
        cache.update(&e);
    }
    cache
}

fn all() -> CacheConfig {
    CacheConfig::new()
        .resources(ResourceType::all())
        .parts(CacheParts::all())
        .message_cache_size(10)
}

#[test]
fn default_config_stores_nothing() {
    let cache = replay(CacheConfig::default());
    assert_eq!(cache.stats(), CacheStats::default());
    assert!(cache.current_user().is_none());
    assert_eq!(CacheConfig::default().wanted_events(), [] as [&str; 0]);
}

#[test]
fn full_replay_snapshot() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    assert_eq!(
        cache.stats(),
        CacheStats {
            guilds: 1,
            unavailable_guilds: 0,
            channels: 1,
            threads: 1,
            users: 2,
            members: 2,
            roles: 1,
            emojis: 1,
            stickers: 1,
            soundboard_sounds: 1,
            voice_states: 1,
            presences: 1,
            stage_instances: 1,
            scheduled_events: 1,
            messages: 2,
        }
    );
    assert_eq!(cache.current_user().unwrap().id.get(), BOT);
    let guild = cache.guild(g).unwrap();
    assert!(guild.roles.is_empty(), "roles live in their own store");
    assert!(!guild.features.is_empty());
    let channel = cache.channel(Id::new(TEXT).unwrap()).unwrap();
    assert_eq!(
        channel.guild_id,
        Some(g),
        "GUILD_CREATE channels get their guild id"
    );
    let alice = cache.member(g, Id::new(ALICE).unwrap()).unwrap();
    assert!(
        alice.user.is_none(),
        "users are stored once, not inside members"
    );
    assert_eq!(alice.roles.len(), 1);
    assert_eq!(cache.user(Id::new(ALICE).unwrap()).unwrap().id.get(), ALICE);
    let msgs = cache.channel_messages(Id::new(TEXT).unwrap());
    assert_eq!(msgs.iter().map(|m| m.id.get()).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(&*msgs[0].content, "Supa Hot");
}

/// Count of the resource in the stats (users: `None` = depends on members).
fn count(stats: &CacheStats, r: ResourceType) -> usize {
    match r {
        ResourceType::GUILDS => stats.guilds,
        ResourceType::CHANNELS => stats.channels,
        ResourceType::THREADS => stats.threads,
        ResourceType::USERS => stats.users,
        ResourceType::MEMBERS => stats.members,
        ResourceType::ROLES => stats.roles,
        ResourceType::EMOJIS => stats.emojis,
        ResourceType::STICKERS => stats.stickers,
        ResourceType::SOUNDBOARD_SOUNDS => stats.soundboard_sounds,
        ResourceType::VOICE_STATES => stats.voice_states,
        ResourceType::PRESENCES => stats.presences,
        ResourceType::STAGE_INSTANCES => stats.stage_instances,
        ResourceType::SCHEDULED_EVENTS => stats.scheduled_events,
        ResourceType::MESSAGES => stats.messages,
        _ => unreachable!(),
    }
}

#[test]
fn every_resource_flag_works_on_its_own() {
    for flag in ResourceType::all().iter() {
        let cache = replay(CacheConfig::new().resources(flag).message_cache_size(10));
        let stats = cache.stats();
        for other in ResourceType::all().iter() {
            let n = count(&stats, other);
            if other == flag {
                if flag == ResourceType::USERS {
                    // Without members only the current user is kept.
                    assert_eq!(n, 0, "{flag:?}");
                    assert!(cache.current_user().is_some());
                } else {
                    assert!(n > 0, "{flag:?} on but nothing stored");
                }
            } else {
                assert_eq!(n, 0, "{other:?} stored while only {flag:?} is on");
            }
        }
        if flag != ResourceType::USERS {
            assert!(cache.current_user().is_none(), "{flag:?}");
        }
    }
}

#[test]
fn every_part_flag_works_on_its_own() {
    let g = Id::new(GUILD).unwrap();
    let alice = Id::new(ALICE).unwrap();
    let text = Id::new(TEXT).unwrap();
    let base = CacheConfig::new()
        .resources(ResourceType::all())
        .message_cache_size(10);
    let off = replay(base.clone());
    let on = |p: CacheParts| replay(base.clone().parts(p));

    let check = |part: CacheParts, has: &dyn Fn(&InMemoryCache) -> bool| {
        assert!(!has(&off), "{part:?} data kept while off");
        assert!(has(&on(part)), "{part:?} data missing while on");
    };
    check(CacheParts::MEMBER_ROLES, &|c| {
        !c.member(g, alice).unwrap().roles.is_empty()
    });
    check(CacheParts::MEMBER_PROFILE, &|c| {
        c.member(g, alice).unwrap().nick.is_some()
    });
    check(CacheParts::USER_PROFILE, &|c| {
        c.user(alice).unwrap().avatar.is_some()
    });
    check(CacheParts::MESSAGE_CONTENT, &|c| {
        !c.channel_messages(text)[0].content.is_empty()
    });
    check(CacheParts::MESSAGE_EMBEDS, &|c| {
        !c.channel_messages(text)[0].embeds.is_empty()
    });
    check(CacheParts::MESSAGE_ATTACHMENTS, &|c| {
        !c.channel_messages(text)[0].attachments.is_empty()
    });
    check(CacheParts::GUILD_FEATURES, &|c| {
        !c.guild(g).unwrap().features.is_empty()
    });
    check(CacheParts::CHANNEL_OVERWRITES, &|c| {
        !c.channel(text).unwrap().permission_overwrites.is_empty()
    });
    check(CacheParts::VOICE_STATE_MEMBER, &|c| {
        c.voice_state(g, alice).unwrap().member.is_some()
    });
    check(CacheParts::PRESENCE_ACTIVITIES, &|c| {
        !c.presence(g, alice).unwrap().activities.is_empty()
    });
}

#[test]
fn switching_off_at_runtime_drops_data() {
    let cache = replay(all());
    let text = Id::new(TEXT).unwrap();
    cache.set_config(all().without_parts(CacheParts::MESSAGE_CONTENT));
    assert!(cache.channel_messages(text)[0].content.is_empty());
    assert_eq!(cache.stats().messages, 2);

    cache.set_config(all().without_resources(ResourceType::MEMBERS | ResourceType::MESSAGES));
    let stats = cache.stats();
    assert_eq!((stats.members, stats.messages), (0, 0));
    assert_eq!(stats.guilds, 1, "other resources stay");

    cache.set_config(CacheConfig::default());
    assert_eq!(cache.stats(), CacheStats::default());
    assert!(cache.current_user().is_none());

    // Switching on again fills from the next events only.
    cache.set_config(all());
    assert_eq!(cache.stats().guilds, 0);
    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert_eq!(cache.stats().guilds, 1);
}

#[test]
fn guild_delete_unavailable_keeps_data_removal_cascades() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    cache.update(&ev(
        "GUILD_DELETE",
        &json!({"id": id(GUILD), "unavailable": true}),
    ));
    assert!(cache.is_unavailable(g));
    assert!(cache.guild(g).is_some());

    cache.update(&ev("GUILD_DELETE", &json!({"id": id(GUILD)})));
    let stats = cache.stats();
    assert_eq!(
        stats,
        CacheStats {
            users: 0,
            ..CacheStats::default()
        }
    );
    assert!(
        cache.current_user().is_some(),
        "the bot user is not guild data"
    );
}

#[test]
fn limits() {
    let text = Id::new(TEXT).unwrap();
    let cache = replay(all().message_cache_size(1));
    let msgs = cache.channel_messages(text);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id.get(), 2, "ring buffer keeps the newest");

    let cache = replay(
        all()
            .max_members_per_guild(Some(1))
            .max_presences_per_guild(Some(0)),
    );
    assert_eq!(cache.stats().members, 1);
    assert_eq!(cache.stats().users, 1);
    assert_eq!(cache.stats().presences, 0);

    let cache = replay(all());
    cache.set_config(all().message_cache_size(1).max_members_per_guild(Some(1)));
    assert_eq!(cache.stats().messages, 1);
    assert_eq!(cache.stats().members, 1);
}

#[test]
fn member_lifecycle_and_user_release() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let bob = Id::new(BOB).unwrap();
    let mut update = json!({"guild_id": id(GUILD), "roles": [], "user": user(BOB), "nick": "bobby",
        "joined_at": "2024-01-01T00:00:00.000000+00:00", "flags": 0});
    update["avatar"] = Value::Null;
    cache.update(&ev("GUILD_MEMBER_UPDATE", &update));
    let m = cache.member(g, bob).unwrap();
    assert_eq!(m.nick.as_deref(), Some("bobby"));
    assert!(m.roles.is_empty());

    cache.update(&ev(
        "GUILD_MEMBER_REMOVE",
        &json!({"guild_id": id(GUILD), "user": user(BOB)}),
    ));
    assert!(cache.member(g, bob).is_none());
    assert!(
        cache.user(bob).is_none(),
        "user dropped with its last member entry"
    );

    let mut add = member(BOB);
    add["guild_id"] = id(GUILD);
    cache.update(&ev("GUILD_MEMBER_ADD", &add));
    assert!(cache.member(g, bob).is_some());
    assert!(cache.user(bob).is_some());
}

#[test]
fn channel_thread_message_events() {
    let cache = replay(all());
    let text = Id::new(TEXT).unwrap();
    let mut edited = message(2, TEXT);
    edited["content"] = json!("edited");
    cache.update(&ev("MESSAGE_UPDATE", &edited));
    assert_eq!(
        &*cache.message(text, Id::new(2).unwrap()).unwrap().content,
        "edited"
    );

    cache.update(&ev(
        "MESSAGE_DELETE",
        &json!({"id": "1", "channel_id": id(TEXT)}),
    ));
    assert_eq!(cache.stats().messages, 1);
    cache.update(&ev(
        "MESSAGE_DELETE_BULK",
        &json!({"ids": ["2"], "channel_id": id(TEXT)}),
    ));
    assert_eq!(cache.stats().messages, 0);

    cache.update(&ev(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": id(GUILD), "threads": [], "members": []}),
    ));
    assert_eq!(
        cache.stats().threads,
        0,
        "full sync without threads clears them"
    );

    let mut ch = fixture("text");
    ch["id"] = id(TEXT);
    ch["guild_id"] = id(GUILD);
    cache.update(&ev("CHANNEL_DELETE", &ch));
    assert!(cache.channel(text).is_none());
}

#[test]
fn concurrent_reads_and_updates() {
    let cache = Arc::new(InMemoryCache::new(all()));
    let events = Arc::new(stream());
    let g = Id::new(GUILD).unwrap();
    let mut handles = Vec::new();
    for i in 0..8 {
        let cache = Arc::clone(&cache);
        let events = Arc::clone(&events);
        handles.push(std::thread::spawn(move || {
            for _ in 0..200 {
                if i % 2 == 0 {
                    for e in events.iter() {
                        cache.update(e);
                    }
                } else {
                    let _ = cache.guild(g);
                    let _ = cache.guild_members(g);
                    let _ = cache.stats();
                    if i == 3 {
                        cache.set_config(all().message_cache_size(5));
                    }
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(cache.stats().guilds, 1);
    assert_eq!(cache.stats().members, 2);
}

#[test]
fn validate_and_wanted_events() {
    let c = CacheConfig::new()
        .resources(ResourceType::MEMBERS | ResourceType::MESSAGES)
        .parts(CacheParts::PRESENCE_ACTIVITIES);
    let w = c.validate();
    assert!(w.contains(&ConfigWarning::MembersWithoutUsers));
    assert!(w.contains(&ConfigWarning::MessagesWithoutSize));
    assert!(
        w.iter()
            .any(|w| matches!(w, ConfigWarning::PartWithoutResource { .. }))
    );
    assert!(c.enabled_resources().contains(ResourceType::MEMBERS));
    assert!(
        !c.enabled_resources().contains(ResourceType::USERS),
        "nothing switched on implicitly"
    );

    let events = CacheConfig::guilds_and_channels().wanted_events();
    for name in [
        "READY",
        "GUILD_CREATE",
        "GUILD_DELETE",
        "GUILD_UPDATE",
        "CHANNEL_CREATE",
    ] {
        assert!(events.contains(&name), "{name}");
    }
    assert!(!events.contains(&"MESSAGE_CREATE"));
    assert!(!events.contains(&"PRESENCE_UPDATE"));
}

#[test]
fn insert_messages_merges_limits_and_respects_switches() {
    let text = Id::new(TEXT).unwrap();
    let parse = |n: u64, channel: u64| -> cordex_model::message::Message {
        serde_json::from_value(message(n, channel)).unwrap()
    };
    let cache = InMemoryCache::new(all().message_cache_size(3));
    cache.update(&ev("MESSAGE_CREATE", &message(10, TEXT)));
    // 10 is already cached (live wins), 99 belongs to another channel.
    let added = cache.insert_messages(
        text,
        [7, 8, 9, 10]
            .map(|n| parse(n, TEXT))
            .into_iter()
            .chain([parse(99, 1)]),
    );
    assert_eq!(added, 2, "7 falls out of the 3-message window");
    let ids: Vec<u64> = cache
        .channel_messages(text)
        .iter()
        .map(|m| m.id.get())
        .collect();
    assert_eq!(ids, [8, 9, 10]);

    let no_content = InMemoryCache::new(all().without_parts(CacheParts::MESSAGE_CONTENT));
    no_content.insert_messages(text, [parse(1, TEXT)]);
    assert_eq!(&*no_content.channel_messages(text)[0].content, "");

    let off = InMemoryCache::new(CacheConfig::default());
    assert_eq!(off.insert_messages(text, [parse(1, TEXT)]), 0);
    assert_eq!(off.stats(), CacheStats::default());
}

#[test]
fn insert_members_respects_switches_and_cap() {
    let g = Id::new(GUILD).unwrap();
    let parse =
        |n: u64| -> cordex_model::guild::Member { serde_json::from_value(member(n)).unwrap() };
    let cache = InMemoryCache::new(all().max_members_per_guild(Some(2)));
    assert_eq!(cache.insert_members(g, [parse(1), parse(2), parse(3)]), 2);
    assert_eq!(cache.stats().users, 2);
    let off = InMemoryCache::new(CacheConfig::new().resources(ResourceType::USERS));
    assert_eq!(off.insert_members(g, [parse(1)]), 0);
    assert_eq!(off.stats().users, 0);
}

#[test]
fn permissions_from_cache() {
    use cordex_model::Permissions;

    use crate::PermissionCacheError;

    let mut g = guild_create();
    g["owner_id"] = id(BOT);
    let mut everyone = fixture("role");
    everyone["id"] = id(GUILD);
    everyone["permissions"] = json!("3072"); // VIEW_CHANNEL | SEND_MESSAGES
    let mut role = fixture("role");
    role["id"] = id(10);
    role["permissions"] = json!("0");
    g["roles"] = json!([everyone, role]);
    let cache = InMemoryCache::new(all());
    cache.update(&ev("GUILD_CREATE", &g));

    let (guild, alice) = (Id::new(GUILD).unwrap(), Id::new(ALICE).unwrap());
    // The text channel denies SEND_MESSAGES to @everyone.
    let text = cache
        .permissions(guild, alice, Id::new(TEXT).unwrap())
        .unwrap();
    assert_eq!(text, Permissions::VIEW_CHANNEL);
    // Threads use the parent channel's overwrites.
    let thread = cache
        .permissions(guild, alice, Id::new(THREAD).unwrap())
        .unwrap();
    assert_eq!(thread, Permissions::VIEW_CHANNEL);
    // The owner gets everything.
    let owner = cache
        .permissions(guild, Id::new(BOT).unwrap(), Id::new(TEXT).unwrap())
        .unwrap_err();
    assert_eq!(owner, PermissionCacheError::Member); // owner is not a cached member here

    let no_roles = InMemoryCache::new(all().without_parts(CacheParts::MEMBER_ROLES));
    no_roles.update(&ev("GUILD_CREATE", &g));
    assert_eq!(
        no_roles.permissions(guild, alice, Id::new(TEXT).unwrap()),
        Err(PermissionCacheError::MemberRolesOff)
    );
    let no_overwrites = InMemoryCache::new(all().without_parts(CacheParts::CHANNEL_OVERWRITES));
    no_overwrites.update(&ev("GUILD_CREATE", &g));
    assert_eq!(
        no_overwrites.permissions(guild, alice, Id::new(TEXT).unwrap()),
        Err(PermissionCacheError::OverwritesOff)
    );
}

#[test]
fn interaction_member_is_cached() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let carol = Id::new(777).unwrap();
    assert!(cache.member(g, carol).is_none());
    let interaction = json!({"id": id(1), "application_id": id(BOT), "type": 2, "token": "t",
        "version": 1, "guild_id": id(GUILD), "member": member(777),
        "data": {"id": id(3), "name": "perms", "type": 1}});
    cache.update(&ev("INTERACTION_CREATE", &interaction));
    assert_eq!(
        cache.member(g, carol).unwrap().nick.as_deref(),
        Some("nick")
    );

    // DMs (no guild) and a disabled member resource store nothing.
    let mut dm = interaction.clone();
    dm.as_object_mut().unwrap().remove("guild_id");
    dm["member"] = member(778);
    cache.update(&ev("INTERACTION_CREATE", &dm));
    assert!(cache.member(g, Id::new(778).unwrap()).is_none());
    let off = replay(CacheConfig::new().resources(ResourceType::GUILDS));
    off.update(&ev("INTERACTION_CREATE", &interaction));
    assert!(off.member(g, carol).is_none());
}

#[test]
fn guild_create_replaces_old_channels_and_threads() {
    let g = Id::new(GUILD).unwrap();
    let mut stale = fixture("text");
    stale["id"] = id(150);
    stale["guild_id"] = id(GUILD);
    let mut stale_thread = fixture("thread");
    stale_thread["id"] = id(250);
    stale_thread["guild_id"] = id(GUILD);

    // A channel cached before the first GUILD_CREATE, and one cached between
    // two GUILD_CREATEs: both are gone after the next GUILD_CREATE.
    let cache = InMemoryCache::new(all());
    cache.update(&ev("CHANNEL_CREATE", &stale));
    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert!(cache.channel(Id::new(150).unwrap()).is_none());
    cache.update(&ev("THREAD_CREATE", &stale_thread));
    assert!(cache.thread(Id::new(250).unwrap()).is_some());
    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert!(cache.thread(Id::new(250).unwrap()).is_none());
    assert_eq!(cache.guild_channels(g).len(), 1);
    assert_eq!(cache.guild_threads(g).len(), 1);

    // After the guild is gone, a new GUILD_CREATE starts clean.
    cache.update(&ev("GUILD_DELETE", &json!({"id": id(GUILD)})));
    assert_eq!(cache.guild_channels(g).len(), 0);
    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert_eq!(cache.guild_channels(g).len(), 1);
    assert_eq!(cache.stats().members, 2);
}

#[test]
fn lazy_skips_guild_create_and_caches_on_first_sight() {
    use crate::CachePolicy;

    let g = Id::new(GUILD).unwrap();
    let alice = Id::new(ALICE).unwrap();
    let bob = Id::new(BOB).unwrap();
    let carol = Id::new(777).unwrap();

    let cfg = CacheConfig::new()
        .lazy(ResourceType::all())
        .parts(CacheParts::MEMBER_ROLES | CacheParts::MEMBER_PROFILE)
        .message_cache_size(5);
    assert_eq!(cfg.policy(ResourceType::MEMBERS), CachePolicy::Lazy);
    assert!(cfg.is_lazy(ResourceType::GUILDS));
    assert!(!cfg.is_eager(ResourceType::GUILDS));
    assert!(cfg.has(ResourceType::GUILDS));

    let cache = InMemoryCache::new(cfg);
    // READY and GUILD_CREATE bulk dumps must not populate lazy resources
    // (only the bot's own current_user is kept from READY).
    cache.update(&ev("READY", &ready()));
    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert_eq!(cache.stats(), CacheStats::default());
    assert!(cache.current_user().is_some());

    // Bulk member chunks and thread list syncs are also skipped in lazy mode.
    cache.update(&ev(
        "GUILD_MEMBERS_CHUNK",
        &json!({"guild_id": id(GUILD), "members": [member(ALICE)], "chunk_index": 0, "chunk_count": 1}),
    ));
    cache.update(&ev(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": id(GUILD), "threads": [fixture("thread")], "members": []}),
    ));
    assert_eq!(cache.stats().members, 0);
    assert_eq!(cache.stats().threads, 0);

    // Live MESSAGE_CREATE with a member object caches the author + member
    // (and the message itself).
    let mut msg = message(1, TEXT);
    let mut msg_member = member(ALICE);
    msg_member.as_object_mut().unwrap().remove("user");
    msg["member"] = msg_member;
    cache.update(&ev("MESSAGE_CREATE", &msg));
    assert_eq!(cache.stats().messages, 1);
    assert_eq!(cache.stats().members, 1);
    assert_eq!(cache.stats().users, 1);
    assert_eq!(
        cache.member(g, alice).unwrap().nick.as_deref(),
        Some("nick")
    );
    assert_eq!(cache.user(alice).unwrap().id, alice);

    // Webhook messages must not cache fake webhook authors as members/users.
    let mut wh = msg.clone();
    wh["id"] = id(2);
    wh["author"] = user(888);
    wh["webhook_id"] = id(888);
    cache.update(&ev("MESSAGE_CREATE", &wh));
    assert!(cache.member(g, Id::new(888).unwrap()).is_none());
    assert!(cache.user(Id::new(888).unwrap()).is_none());

    // Live GUILD_MEMBER_UPDATE inserts an unseen member in lazy mode.
    let update = json!({"guild_id": id(GUILD), "roles": [id(10)], "user": user(BOB), "nick": "bobby",
        "joined_at": "2024-01-01T00:00:00.000000+00:00", "flags": 0});
    cache.update(&ev("GUILD_MEMBER_UPDATE", &update));
    assert_eq!(cache.member(g, bob).unwrap().nick.as_deref(), Some("bobby"));
    assert!(cache.user(bob).is_some());

    // Live VOICE_STATE_UPDATE with member caches both voice state and member.
    cache.update(&ev(
        "VOICE_STATE_UPDATE",
        &json!({
            "guild_id": id(GUILD), "user_id": id(777), "channel_id": id(TEXT), "session_id": "s",
            "deaf": false, "mute": false, "self_deaf": false, "self_mute": false,
            "self_video": false, "suppress": false, "member": member(777)
        }),
    ));
    assert!(cache.voice_state(g, carol).is_some());
    assert!(cache.member(g, carol).is_some());
    assert!(cache.user(carol).is_some());

    // Live GUILD_UPDATE / CHANNEL_UPDATE / GUILD_ROLE_UPDATE cache on sight.
    let mut guild_upd = fixture("guild");
    guild_upd["id"] = id(GUILD);
    cache.update(&ev("GUILD_UPDATE", &guild_upd));
    let mut ch = fixture("text");
    ch["id"] = id(TEXT);
    ch["guild_id"] = id(GUILD);
    cache.update(&ev("CHANNEL_UPDATE", &ch));
    let mut role = fixture("role");
    role["id"] = id(10);
    cache.update(&ev(
        "GUILD_ROLE_UPDATE",
        &json!({"guild_id": id(GUILD), "role": role}),
    ));
    assert!(cache.guild(g).is_some());
    assert!(cache.channel(Id::new(TEXT).unwrap()).is_some());
    assert!(cache.role(g, Id::new(10).unwrap()).is_some());

    // GUILD_DELETE still cleans everything up.
    cache.update(&ev("GUILD_DELETE", &json!({"id": id(GUILD)})));
    assert_eq!(cache.stats(), CacheStats::default());
}

#[test]
fn lazy_users_with_eager_members_skips_bulk_users() {
    let g = Id::new(GUILD).unwrap();
    let alice = Id::new(ALICE).unwrap();
    let cfg = CacheConfig::new()
        .resources(ResourceType::MEMBERS)
        .lazy(ResourceType::USERS);
    let cache = InMemoryCache::new(cfg);

    cache.update(&ev("GUILD_CREATE", &guild_create()));
    assert_eq!(cache.stats().members, 2, "members are eager");
    assert_eq!(
        cache.stats().users,
        0,
        "users are lazy, skipped in GUILD_CREATE"
    );

    // Once Alice sends a message with member info, Alice's user is cached.
    let mut msg = message(1, TEXT);
    let mut msg_member = member(ALICE);
    msg_member.as_object_mut().unwrap().remove("user");
    msg["member"] = msg_member;
    cache.update(&ev("MESSAGE_CREATE", &msg));
    assert_eq!(cache.stats().users, 1);
    assert!(cache.user(alice).is_some());
    assert!(cache.member(g, alice).is_some());
}

#[test]
fn policy_transitions_and_lazy_wanted_events() {
    use crate::CachePolicy;

    let cfg = CacheConfig::new()
        .resources(ResourceType::GUILDS | ResourceType::MEMBERS)
        .lazy(ResourceType::MEMBERS)
        .with_policy(ResourceType::ROLES, CachePolicy::Lazy);
    assert_eq!(cfg.policy(ResourceType::GUILDS), CachePolicy::Eager);
    assert_eq!(cfg.policy(ResourceType::MEMBERS), CachePolicy::Lazy);
    assert_eq!(cfg.policy(ResourceType::ROLES), CachePolicy::Lazy);
    assert_eq!(cfg.policy(ResourceType::CHANNELS), CachePolicy::Off);
    assert_eq!(cfg.eager_resources(), ResourceType::GUILDS);
    assert_eq!(
        cfg.lazy_resources(),
        ResourceType::MEMBERS | ResourceType::ROLES
    );

    // Pure lazy config omits bulk events (GUILD_CREATE, GUILD_MEMBERS_CHUNK, THREAD_LIST_SYNC).
    let lazy_only = CacheConfig::new().lazy(ResourceType::MEMBERS | ResourceType::THREADS);
    let events = lazy_only.wanted_events();
    assert!(!events.contains(&"GUILD_CREATE"));
    assert!(!events.contains(&"GUILD_MEMBERS_CHUNK"));
    assert!(!events.contains(&"THREAD_LIST_SYNC"));
    assert!(events.contains(&"GUILD_DELETE"));
    assert!(events.contains(&"MESSAGE_CREATE"));
    assert!(events.contains(&"INTERACTION_CREATE"));
    assert!(events.contains(&"THREAD_CREATE"));

    // Runtime downgrade Eager -> Lazy keeps existing entries; -> Off drops them.
    let cache = replay(all());
    assert_eq!(cache.stats().members, 2);
    cache.set_config(all().lazy(ResourceType::MEMBERS));
    assert_eq!(cache.stats().members, 2);
    cache.set_config(all().with_policy(ResourceType::MEMBERS, CachePolicy::Off));
    assert_eq!(cache.stats().members, 0);
}

#[test]
fn member_count_and_member_remove_cascade() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let alice = Id::new(ALICE).unwrap();
    let carol = Id::new(777).unwrap();

    assert_eq!(cache.guild_member_count(g), Some(2));
    assert!(cache.voice_state(g, alice).is_some());
    assert!(cache.presence(g, alice).is_some());

    // Member add increments member_count.
    let mut new_m = member(777);
    new_m["guild_id"] = id(GUILD);
    cache.update(&ev("GUILD_MEMBER_ADD", &new_m));
    assert_eq!(cache.guild_member_count(g), Some(3));
    assert!(cache.member(g, carol).is_some());

    // Member remove decrements member_count and drops voice state + presence.
    cache.update(&ev(
        "GUILD_MEMBER_REMOVE",
        &json!({"guild_id": id(GUILD), "user": user(ALICE)}),
    ));
    assert_eq!(cache.guild_member_count(g), Some(2));
    assert!(cache.member(g, alice).is_none());
    assert!(cache.voice_state(g, alice).is_none());
    assert!(cache.presence(g, alice).is_none());

    // Guild delete clears member_count.
    cache.update(&ev("GUILD_DELETE", &json!({"id": id(GUILD)})));
    assert_eq!(cache.guild_member_count(g), None);
}

#[test]
fn role_delete_cascades_to_members_and_channel_overwrites() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let ch = Id::new(TEXT).unwrap();
    let alice = Id::new(ALICE).unwrap();
    let role_id = Id::new(10).unwrap();

    // Add a role overwrite for role 10 on TEXT channel.
    let mut text = fixture("text");
    text["id"] = id(TEXT);
    text["guild_id"] = id(GUILD);
    text["permission_overwrites"] = json!([
        {"id": id(GUILD), "type": 0, "allow": "0", "deny": "2048"},
        {"id": id(10), "type": 0, "allow": "2048", "deny": "0"}
    ]);
    cache.update(&ev("CHANNEL_UPDATE", &text));
    assert_eq!(cache.channel(ch).unwrap().permission_overwrites.len(), 2);
    assert!(cache.member(g, alice).unwrap().roles.contains(&role_id));

    // Deleting role 10 removes it from roles, member.roles, and channel overwrites.
    cache.update(&ev(
        "GUILD_ROLE_DELETE",
        &json!({"guild_id": id(GUILD), "role_id": id(10)}),
    ));
    assert!(cache.role(g, role_id).is_none());
    assert!(!cache.member(g, alice).unwrap().roles.contains(&role_id));
    assert_eq!(cache.channel(ch).unwrap().permission_overwrites.len(), 1);
}

#[test]
fn channel_delete_cascades_to_threads_voice_and_stage() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let text = Id::new(TEXT).unwrap();
    let thread = Id::new(THREAD).unwrap();
    let alice = Id::new(ALICE).unwrap();

    // Put a message into the child thread and align stage_instance channel_id with TEXT.
    cache.update(&ev("MESSAGE_CREATE", &message(99, THREAD)));
    let mut stage = fixture("stage");
    stage["id"] = id(300);
    stage["guild_id"] = id(GUILD);
    stage["channel_id"] = id(TEXT);
    cache.update(&ev("STAGE_INSTANCE_UPDATE", &stage));

    assert!(cache.thread(thread).is_some());
    assert_eq!(cache.channel_messages(thread).len(), 1);
    assert!(cache.voice_state(g, alice).is_some());
    assert!(cache.stage_instance(g, Id::new(300).unwrap()).is_some());

    // Deleting parent channel TEXT drops child thread, its messages, voice state and stage instance.
    let mut ch = fixture("text");
    ch["id"] = id(TEXT);
    ch["guild_id"] = id(GUILD);
    cache.update(&ev("CHANNEL_DELETE", &ch));

    assert!(cache.channel(text).is_none());
    assert!(cache.thread(thread).is_none());
    assert_eq!(cache.channel_messages(thread).len(), 0);
    assert!(cache.voice_state(g, alice).is_none());
    assert!(cache.stage_instance(g, Id::new(300).unwrap()).is_none());
}

#[test]
fn incremental_field_updates_for_channels_threads_events_and_users() {
    let cache = replay(all());
    let g = Id::new(GUILD).unwrap();
    let text = Id::new(TEXT).unwrap();
    let thread = Id::new(THREAD).unwrap();
    let sched = Id::new(350).unwrap();

    // MESSAGE_CREATE updates last_message_id on channel and thread.
    assert_eq!(
        cache.channel(text).unwrap().last_message_id,
        Some(Id::new(2).unwrap())
    );
    cache.update(&ev("MESSAGE_CREATE", &message(42, THREAD)));
    assert_eq!(
        cache.thread(thread).unwrap().last_message_id,
        Some(Id::new(42).unwrap())
    );

    // CHANNEL_PINS_UPDATE updates last_pin_timestamp on channel and thread.
    cache.update(&ev(
        "CHANNEL_PINS_UPDATE",
        &json!({"guild_id": id(GUILD), "channel_id": id(TEXT), "last_pin_timestamp": "2026-01-01T00:00:00+00:00"}),
    ));
    assert!(cache.channel(text).unwrap().last_pin_timestamp.is_some());

    // THREAD_MEMBERS_UPDATE updates thread member_count.
    cache.update(&ev(
        "THREAD_MEMBERS_UPDATE",
        &json!({"id": id(THREAD), "guild_id": id(GUILD), "member_count": 7}),
    ));
    assert_eq!(cache.thread(thread).unwrap().member_count, Some(7));

    // GUILD_SCHEDULED_EVENT_USER_ADD / REMOVE updates user_count.
    cache.update(&ev(
        "GUILD_SCHEDULED_EVENT_USER_ADD",
        &json!({"guild_scheduled_event_id": id(350), "user_id": id(ALICE), "guild_id": id(GUILD)}),
    ));
    assert_eq!(cache.scheduled_event(g, sched).unwrap().user_count, Some(1));
    cache.update(&ev(
        "GUILD_SCHEDULED_EVENT_USER_REMOVE",
        &json!({"guild_scheduled_event_id": id(350), "user_id": id(ALICE), "guild_id": id(GUILD)}),
    ));
    assert_eq!(cache.scheduled_event(g, sched).unwrap().user_count, Some(0));

    // USER_UPDATE updates both current_user and self.users if bot is a cached member.
    let mut bot_m = member(BOT);
    bot_m["guild_id"] = id(GUILD);
    cache.update(&ev("GUILD_MEMBER_ADD", &bot_m));
    let mut bot_u = user(BOT);
    bot_u["username"] = json!("renamed-bot");
    cache.update(&ev("USER_UPDATE", &bot_u));
    assert_eq!(&*cache.current_user().unwrap().username, "renamed-bot");
    assert_eq!(
        &*cache.user(Id::new(BOT).unwrap()).unwrap().username,
        "renamed-bot"
    );
}

#[test]
fn message_reactions_are_maintained_in_cache() {
    let cache = replay(all());
    let ch = Id::new(TEXT).unwrap();
    let msg_id = Id::new(1).unwrap();
    let initial = cache.message(ch, msg_id).unwrap().reactions.len();

    // Add normal reaction by Alice, then by Bot.
    cache.update(&ev(
        "MESSAGE_REACTION_ADD",
        &json!({
            "user_id": id(ALICE), "channel_id": id(TEXT), "message_id": id(1),
            "guild_id": id(GUILD), "emoji": {"id": null, "name": "🎉"}
        }),
    ));
    cache.update(&ev(
        "MESSAGE_REACTION_ADD",
        &json!({
            "user_id": id(BOT), "channel_id": id(TEXT), "message_id": id(1),
            "guild_id": id(GUILD), "emoji": {"id": null, "name": "🎉"}
        }),
    ));
    let m = cache.message(ch, msg_id).unwrap();
    assert_eq!(m.reactions.len(), initial + 1);
    let party = m
        .reactions
        .iter()
        .find(|r| r.emoji.name.as_deref() == Some("🎉"))
        .unwrap();
    assert_eq!(party.count, 2);
    assert_eq!(party.count_details.normal, 2);
    assert!(party.me);

    // Add a second custom emoji reaction.
    cache.update(&ev(
        "MESSAGE_REACTION_ADD",
        &json!({
            "user_id": id(ALICE), "channel_id": id(TEXT), "message_id": id(1),
            "guild_id": id(GUILD), "emoji": {"id": id(20), "name": "custom"}
        }),
    ));
    assert_eq!(
        cache.message(ch, msg_id).unwrap().reactions.len(),
        initial + 2
    );

    // Bot removes its reaction -> count drops to 1 and `me` becomes false.
    cache.update(&ev(
        "MESSAGE_REACTION_REMOVE",
        &json!({
            "user_id": id(BOT), "channel_id": id(TEXT), "message_id": id(1),
            "guild_id": id(GUILD), "emoji": {"id": null, "name": "🎉"}
        }),
    ));
    let m = cache.message(ch, msg_id).unwrap();
    let party = m
        .reactions
        .iter()
        .find(|r| r.emoji.name.as_deref() == Some("🎉"))
        .unwrap();
    assert_eq!(party.count, 1);
    assert!(!party.me);

    // Remove all of one emoji.
    cache.update(&ev(
        "MESSAGE_REACTION_REMOVE_EMOJI",
        &json!({
            "channel_id": id(TEXT), "message_id": id(1),
            "guild_id": id(GUILD), "emoji": {"id": id(20), "name": "custom"}
        }),
    ));
    assert_eq!(
        cache.message(ch, msg_id).unwrap().reactions.len(),
        initial + 1
    );

    // Remove all reactions.
    cache.update(&ev(
        "MESSAGE_REACTION_REMOVE_ALL",
        &json!({"channel_id": id(TEXT), "message_id": id(1), "guild_id": id(GUILD)}),
    ));
    assert_eq!(cache.message(ch, msg_id).unwrap().reactions.len(), 0);
}

#[test]
fn guild_members_with_ids_exposes_user_id_and_bot_flag() {
    let cache = InMemoryCache::new(
        CacheConfig::new().resources(ResourceType::MEMBERS | ResourceType::USERS),
    );
    let g = Id::new(GUILD).unwrap();
    let bot_uid = Id::new(777).unwrap();

    let mut m = member(777);
    m["guild_id"] = id(GUILD);
    m["user"]["bot"] = json!(true);
    cache.update(&ev("GUILD_MEMBER_ADD", &m));

    let list = cache.guild_members_with_ids(g);
    assert_eq!(list.len(), 1);
    let (uid, cached_member) = &list[0];
    assert_eq!(*uid, bot_uid);
    assert!(cached_member.user.is_none());
    assert!(cache.user(*uid).unwrap().bot);
    assert_eq!(cache.guild_member_ids(g), vec![bot_uid]);

    // guild_voice_states returns all cached voice states of a guild.
    let full = replay(all());
    assert_eq!(full.guild_voice_states(g).len(), 1);
}
