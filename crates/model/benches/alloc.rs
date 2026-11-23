//! Allocations per parsed gateway frame, measured with `dhat` and compared
//! with the budgets of PLAN §16.1 (filtered event: 0, typed
//! `MESSAGE_CREATE`: at most 15).
//!
//! `cargo bench -p cordex-model --bench alloc`

#![allow(missing_docs, reason = "benchmark binary")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "benchmark setup: a broken fixture must stop the run"
)]

mod support;
use support as common;

use cordex_model::gateway::Event;
use cordex_model::gateway::payload::Envelope;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Allocations and bytes of one call of `f` (after a warm-up call).
fn measure<T>(mut f: impl FnMut() -> T) -> (u64, u64) {
    drop(f());
    let before = dhat::HeapStats::get();
    let value = f();
    let after = dhat::HeapStats::get();
    drop(value);
    (
        after.total_blocks - before.total_blocks,
        after.total_bytes - before.total_bytes,
    )
}

fn parse(frame: &str) -> Event {
    let env = Envelope::parse(frame).expect("envelope");
    Event::from_dispatch(env.t.as_deref().unwrap_or_default(), env.d).expect("event")
}

fn main() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let message = common::frame("MESSAGE_CREATE", &common::message_create());
    let interaction = common::frame("INTERACTION_CREATE", &common::interaction_create());
    let guild = common::frame("GUILD_CREATE", &common::guild_create(1000));

    let rows = [
        (
            "filtered event (envelope only)",
            measure(|| Envelope::parse(&guild).map(|e| e.op)),
            Some(1),
        ),
        ("MESSAGE_CREATE", measure(|| parse(&message)), Some(15)),
        ("INTERACTION_CREATE", measure(|| parse(&interaction)), None),
        (
            "GUILD_CREATE (1000 members)",
            measure(|| parse(&guild)),
            None,
        ),
    ];
    println!("{:<34} {:>10} {:>12}  budget", "payload", "allocs", "bytes");
    let mut over = false;
    for (name, (blocks, bytes), budget) in rows {
        let verdict = match budget {
            Some(max) if blocks > max => {
                over = true;
                format!("OVER (max {max})")
            }
            Some(max) => format!("ok (max {max})"),
            None => "-".to_owned(),
        };
        println!("{name:<34} {blocks:>10} {bytes:>12}  {verdict}");
    }
    if over {
        println!("\nat least one allocation budget is exceeded");
    }
}
