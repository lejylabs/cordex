# cordex-http

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Discord REST client for [cordex]: all 235 endpoints of the official bot API,
typed, with a rate limiter that keeps your bot's IP safe.

- **Rate limits done right**: per-route buckets, the global limit, and an
  invalid request guard that stops before Discord's Cloudflare ban
  threshold. A 401 poisons the client so a broken token cannot hammer the API.
- **Local validation**: lengths, counts and file sizes are checked before a
  request is sent, with errors that name the problem.
- **Retries** for safe cases (429 with a short wait, 5xx on idempotent
  requests), configurable timeouts and body size caps.
- **Builders that read like sentences**:
  `client.get_audit_log(guild).limit(10).await`.
- **Raw requests** for anything not typed yet (`Client::request`,
  `Route::raw`), with the same rate limiting.
- **Pluggable**: `RateLimiter` trait for a shared limiter across processes,
  3-tier `RequestPriority` (`Critical`, `Normal`, `Background`),
  `default_allowed_mentions`, opt-in `oauth2` client, `#[cfg(unix)]`
  `unix_socket` transport, TCP/HTTP2 keepalive, and mock transport for tests
  (`cordex_http::mock`).

Most bots use the facade crate [`cordex`][facade], which re-exports this crate.

## Example

```rust,no_run
use cordex_http::{Client, Route};
use cordex_model::id::Id;
use cordex_model::id::marker::ChannelMarker;

async fn example(token: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(token)?;
    let channel: Id<ChannelMarker> = "381880193700069377".parse()?;

    // Typed endpoint with local validation.
    let message = client.create_message(channel).content("Hello!")?.send().await?;
    client.create_reaction(channel, message.id, "👋").await?;

    // Raw request for something new, still rate limited.
    let raw: serde_json::Value = client
        .get(Route::raw("/channels/{channel_id}", channel))
        .send_json()
        .await?;
    println!("{}", raw["name"]);
    Ok(())
}
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option. Questions or help: [cordex Discord server][discord link].

[cordex]: https://github.com/lejylabs/cordex
[facade]: https://crates.io/crates/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-http.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-http
[docs badge]: https://img.shields.io/docsrs/cordex-http?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-http
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
