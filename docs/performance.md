# Performance

Measured with `cargo xtask bench` (criterion + dhat). The numbers below are
from a local development machine (Windows 11, release profile with fat LTO,
2026-11-23); they compare runs on the same machine. The official baseline
is measured on the production KVM (PLAN §16.1) and will replace
`benches/baseline.json` once taken there.

## Gateway parsing

Full frame (`{"op":0,"t":…,"d":…}`) to a typed `Event`, what a shard does
for every dispatch.

| Payload | Size | Mean | Target (§16.1) |
|---|---:|---:|---|
| `MESSAGE_CREATE` | 0.6 KiB | **1.41 µs** | p99 < 50 µs |
| `INTERACTION_CREATE` | 0.7 KiB | **1.62 µs** (was 1.82) | – |
| `GUILD_CREATE`, 1000 members, 200 channels, 50 roles, 500 presences | 0.9 MiB | **2.89 ms** (≈ 320 MiB/s) | – |
| Filtered `MESSAGE_CREATE` (envelope only) | 0.6 KiB | **0.36 µs** | < 5 µs |
| Filtered `GUILD_CREATE` (envelope only) | 0.9 MiB | 0.37 ms (≈ 2.4 GiB/s) | – |

A filtered event (not in the `EventTypeFlags`) costs only the envelope scan,
about 8× less than parsing it.

`INTERACTION_CREATE` parses `data` in one pass when `type` comes before it
(Discord's usual key order); before, `data` was always copied into a raw
buffer and parsed a second time. The bench payload now keeps that key order.

## Allocations per frame

| Payload | Allocations | Bytes | Budget (§16.1) |
|---|---:|---:|---|
| Filtered event (envelope only) | 1 | 8 | ≤ 1 |
| `MESSAGE_CREATE` | 6 | 1369 | ≤ 15 |
| `INTERACTION_CREATE` | 12 (was 15) | 1881 | – |
| `GUILD_CREATE` (1000 members) | 8503 (was 9503) | 3.0 MB | – |

The budget for filtered events was 0 in the plan. The one allocation comes
from `serde_json` itself: it keeps an 8-byte stack while skipping a nested
value (the same for `RawValue` and `IgnoredAny`); cordex allocates nothing
there. Removing it would need an own JSON scanner, which is not worth 8
bytes per event.

`User::discriminator` is a `Discriminator(u16)` instead of a string: only
bots still have a legacy tag, everyone else sends `"0"`, which cost one
allocation per user. `GUILD_CREATE` now allocates about 8.5 times per member
in this (rich) fixture; the rest are names, role lists and presence objects.

## Cache

| Operation | Mean | Target (§16.1) |
|---|---:|---|
| Lookup member by id | **22.7 ns** | < 200 ns |
| Lookup channel by id | **18.2 ns** | < 200 ns |
| `update(MESSAGE_CREATE)`, everything on, 100 messages per channel | 208 ns | – |
| `update(GUILD_CREATE)` with 1000 members, everything on | **657 µs** (was 806) | – |
| same, with 100 other guilds (20 000 channels) cached | 668 µs (was 844) | – |
| `update(GUILD_CREATE)` with the cache off (`CacheConfig::default()`) | **8.8 ns** | – |

With the cache off an event costs a few nanoseconds: the cache checks its
switches before doing any work.

`GUILD_CREATE` inserts members and presences with one lock per map instead
of one per entry, reserves map capacity up front, and skips scanning every
cached channel on a guild's first `GUILD_CREATE` (it used to grow with the
number of guilds). The "was" values are the old code measured with the
current bench; the bench no longer times dropping the cache (the first
baseline, 944 µs, included that).

## Binary size

`examples/bot.rs`, release profile with fat LTO: **9.16 MiB** (was 9.85 MiB
before removing `serde(flatten)`/`untagged` from deserialization and parsing
every body from `&str`). About half of a bot binary is serde code of the
models (`cargo bloat`); TLS (`aws-lc-rs`) is 0.2–0.5 MiB.

## Not measured yet

- Rate limiter acquire under contention, zlib-stream inflate, event dispatch
  through the shard task, load replay with p99 latency and RSS (PLAN §16.3).
- The numbers on the production KVM and the 72 h soak test.

## Reproduce

```bash
cargo xtask bench                 # run and compare with benches/baseline.json
cargo xtask bench --save          # make the current run the baseline
cargo xtask bench -- --measurement-time 3   # extra criterion arguments
```
