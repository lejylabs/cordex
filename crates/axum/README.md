# cordex-axum

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Axum integration for [cordex].

- **`VerifiedInteraction`**: Axum extractor that verifies Discord's Ed25519 signature (`X-Signature-Ed25519` + `X-Signature-Timestamp`) and returns `401 Unauthorized` on failure.
- **`interactions_router`**: ready-to-mount `axum::Router` for `POST /` with automatic `PING` → `PONG` handling.
- **`health_router`**: ready-to-mount `/healthz` (`200 OK`) and `/readyz` (`200 OK` / `503 Service Unavailable`) routes backed by `cordex::bot::HealthSnapshot`.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

[cordex]: https://github.com/lejylabs/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-axum.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-axum
[docs badge]: https://img.shields.io/docsrs/cordex-axum?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-axum
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
