# REST Client (`cordex-http`)

`cordex-http` is a typed, rate-limit-aware Discord v10 REST client built on Hyper and `rustls`.

## Creating a Client

When using `Bot`, access the shared client via `ctx.http()`. For standalone REST usage (scripts, web servers, workers):

```rust,no_run
use cordex::http::Client;

let client = Client::new(std::env::var("DISCORD_TOKEN")?);
let me = client.current_user().await?;
println!("Logged in as {}", me.username);
```

## HTTP/2 & TLS

- **HTTP/2 via ALPN** is enabled by default (`http2` feature) with adaptive flow control (`http2_adaptive_window(true)`), falling back to HTTP/1.1 automatically.
- Force HTTP/1.1 at runtime with `ClientBuilder::new(token).http1_only(true)`, or disable the `http2` feature for smaller binaries.
- Uses `rustls` with `aws-lc-rs` and system root certificates (`rustls-platform-verifier`) — no OpenSSL dependency.

## Scoped Audit-Log Reasons

Attach an `X-Audit-Log-Reason` header to one or more moderation calls without mutating the shared client:

```rust,no_run
use cordex::http::Client;

let scoped = client.with_reason("Spam cleanup");
scoped.delete_message(channel_id, message_id).await?;
```

`with_reason` is a cheap clone sharing the same connection pool and rate limiter.

## Application & Interaction Endpoints (`InteractionClient`)

Avoid passing `application_id` repeatedly by creating an `InteractionClient`:

```rust,no_run
let ic = client.interaction_for_current_app().await?;
let commands = ic.global_commands().await?;
```

`InteractionClient` covers global and guild commands (including bulk `set_*_commands`), command permissions, interaction responses and follow-ups, application emojis, entitlements, SKUs, and role connection metadata.

## Rate Limiting & Metrics

- Per-route buckets and global rate limits are handled automatically with retry-after backoff.
- Inspect live counters via `client.metrics()` (`ClientMetrics`: total requests, transport errors, timeouts, 429s by scope, 5xx, active buckets) or export them via the optional `metrics` feature.
