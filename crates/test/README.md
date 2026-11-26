# cordex-test

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Offline testing harness and `.jsonl` gateway-replay runner for [cordex] bots.

- **`TestBot`**: wraps any normal `cordex::bot::Bot` without code changes.
- **In-memory `MockHttp` recorder**: captures all outgoing REST requests with custom response overrides and error injection (e.g. HTTP 403 / 429).
- **Builders & assertions**: `slash`, `click_button`, `submit_modal`, `trigger_autocomplete`, `send_message`, `assert_replied`, `assert_ephemeral`, `assert_deferred`, `assert_modal`, `assert_autocomplete_choices`, `assert_sent_message`.
- **Deterministic time**: works with `#[tokio::test(start_paused = true)]` for auto-defer, cooldowns, timeouts, and `Bot::every` schedulers.
- **Gateway replay**: `replay_jsonl` / `replay_jsonl_str` replays captures recorded with `cordex-gateway`'s `record` feature (`TrafficRecorder` + `Anonymizer`).

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

[cordex]: https://github.com/lejylabs/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-test.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-test
[docs badge]: https://img.shields.io/docsrs/cordex-test?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-test
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
