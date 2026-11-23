//! Gateway parse benchmarks (PLAN §16.3): full frame → typed event, and the
//! cost of an event the filter skips (envelope only).
//!
//! `cargo bench -p cordex-model --bench parse`

#![allow(missing_docs, reason = "benchmark binary")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "benchmark setup: a broken fixture must stop the run"
)]

mod support;
use support as common;

use std::hint::black_box;

use cordex_model::gateway::Event;
use cordex_model::gateway::payload::Envelope;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

/// What the gateway does per frame: envelope, then the typed event.
fn parse_frame(frame: &str) -> Event {
    let env = Envelope::parse(frame).expect("envelope");
    Event::from_dispatch(env.t.as_deref().unwrap_or_default(), env.d).expect("event")
}

fn bench(c: &mut Criterion) {
    let message = common::frame("MESSAGE_CREATE", &common::message_create());
    let interaction = common::frame("INTERACTION_CREATE", &common::interaction_create());
    let guild = common::frame("GUILD_CREATE", &common::guild_create(1000));

    let mut group = c.benchmark_group("gateway_parse");
    for (name, frame) in [
        ("message_create", &message),
        ("interaction_create", &interaction),
        ("guild_create_1000_members", &guild),
    ] {
        group.throughput(Throughput::Bytes(frame.len() as u64));
        group.bench_function(name, |b| b.iter(|| parse_frame(black_box(frame))));
    }
    // Filtered events: only the envelope is read, `d` stays raw.
    group.throughput(Throughput::Bytes(message.len() as u64));
    group.bench_function("filtered_message_create_envelope_only", |b| {
        b.iter(|| Envelope::parse(black_box(&message)).expect("envelope").op);
    });
    group.throughput(Throughput::Bytes(guild.len() as u64));
    group.bench_function("filtered_guild_create_envelope_only", |b| {
        b.iter(|| Envelope::parse(black_box(&guild)).expect("envelope").op);
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
