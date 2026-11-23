# Deployment & Performance

## Release build profile

Recommended settings in your bot's `Cargo.toml`:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = "debuginfo"
# Keep panic = "unwind" so cordex's shard panic guard can recover a single shard.
panic = "unwind"
```

## Allocator

`cordex` never sets a global allocator. In your binary crate, `mimalloc` is a good default for low latency and steady RSS:

```rust,ignore
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

## Observability

- **Tracing:** `cordex` emits structured `tracing` spans (`shard{id=...}`, `http.request{method, route}`) without ever logging tokens.
- **Metrics:** Read live counters without extra dependencies via `ShardStatus::counters` (identifies, resumes, dispatches, dropped events, compressed/decompressed bytes) and `Client::metrics()` (requests, 429s by scope, 5xx, timeouts, invalid-request window, active buckets).
