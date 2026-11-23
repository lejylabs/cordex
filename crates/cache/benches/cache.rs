//! Cache benchmarks (PLAN §16.3): feeding a large `GUILD_CREATE`, a
//! `MESSAGE_CREATE`, and lookups by id (target: < 200 ns).
//!
//! `cargo bench -p cordex-cache --bench cache`

#![allow(missing_docs, reason = "benchmark binary")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "benchmark setup: a broken fixture must stop the run"
)]

#[path = "../../model/benches/support/mod.rs"]
mod common;

use std::hint::black_box;

use cordex_cache::{CacheConfig, CacheParts, InMemoryCache, ResourceType};
use cordex_model::gateway::Event;
use cordex_model::id::Id;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::value::RawValue;

fn event(name: &str, data: String) -> Event {
    let raw = RawValue::from_string(data).expect("raw");
    Event::from_dispatch(name, Some(&raw)).expect("event")
}

fn full() -> CacheConfig {
    CacheConfig::new()
        .resources(ResourceType::all())
        .parts(CacheParts::all())
        .message_cache_size(100)
}

fn bench(c: &mut Criterion) {
    let guild = event("GUILD_CREATE", common::guild_create(1000));
    let message = event("MESSAGE_CREATE", common::message_create());

    let mut group = c.benchmark_group("cache");
    group.bench_function("update_guild_create_1000_members", |b| {
        b.iter_batched(
            || InMemoryCache::new(full()),
            // Return the cache so dropping it is not measured.
            |cache| {
                cache.update(black_box(&guild));
                cache
            },
            BatchSize::SmallInput,
        );
    });
    // A bot in many guilds: 100 guilds (20 000 channels) are cached already.
    let others: Vec<Event> = (1..=100)
        .map(|n| event("GUILD_CREATE", common::guild_create_numbered(10, n)))
        .collect();
    group.bench_function("update_guild_create_after_100_guilds", |b| {
        b.iter_batched(
            || {
                let cache = InMemoryCache::new(full());
                for e in &others {
                    cache.update(e);
                }
                cache
            },
            // Return the cache so dropping it is not measured.
            |cache| {
                cache.update(black_box(&guild));
                cache
            },
            BatchSize::LargeInput,
        );
    });
    group.bench_function("update_guild_create_cache_off", |b| {
        let cache = InMemoryCache::new(CacheConfig::default());
        b.iter(|| cache.update(black_box(&guild)));
    });

    let cache = InMemoryCache::new(full());
    cache.update(&guild);
    group.bench_function("update_message_create", |b| {
        b.iter(|| cache.update(black_box(&message)));
    });

    let guild_id = Id::new(175_928_847_299_117_064).expect("id");
    let user_id = Id::new(175_928_847_299_117_063 + 100_500).expect("id");
    let channel_id = Id::new(175_928_847_299_117_063 + 20_100).expect("id");
    group.bench_function("lookup_member", |b| {
        b.iter(|| cache.member(black_box(guild_id), black_box(user_id)));
    });
    group.bench_function("lookup_channel", |b| {
        b.iter(|| cache.channel(black_box(channel_id)));
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
