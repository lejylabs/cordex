# cordex-cache

Optional in-memory cache for Discord data, fed with gateway events.
Everything is off by default: every resource, every optional part of a
resource and every limit is its own switch, also at runtime.

Part of [cordex](https://github.com/lejylabs/cordex), an independent Discord
bot API library for Rust. Most users enable the `cache` feature of the facade
crate [`cordex`](https://crates.io/crates/cordex) instead of depending on this
one.

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
