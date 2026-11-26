# cordex-macros

[![crates.io][crates badge]][crates link] [![docs.rs][docs badge]][docs link] [![guide](https://img.shields.io/badge/guide-cordex_docs-1f6feb.svg?style=for-the-badge)](https://lejylabs.github.io/cordex/) [![discord][discord badge]][discord link] ![rust 1.95+][rust badge]

Optional derive macros for [cordex] (enabled via `features = ["macros"]`, off by default).

- `#[derive(CommandChoice)]`: turns unit enums into Discord slash-command string choices (`#[choice(name = "...", value = "...")]`).
- `#[derive(CommandArgs)]`: derives typed slash-command options and parsing (`String`, `i64`, `f64`, `bool`, `UserId`, `ChannelId`, `RoleId`, `Option<T>`, `#[arg(choice)]`, `#[arg(description = "...")]`, or doc comments).

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

[cordex]: https://github.com/lejylabs/cordex
[crates badge]: https://img.shields.io/crates/v/cordex-macros.svg?style=for-the-badge&logo=rust
[crates link]: https://crates.io/crates/cordex-macros
[docs badge]: https://img.shields.io/docsrs/cordex-macros?style=for-the-badge&logo=docsdotrs
[docs link]: https://docs.rs/cordex-macros
[discord badge]: https://img.shields.io/badge/discord-join-5865F2.svg?style=for-the-badge&logo=discord&logoColor=white
[discord link]: https://discord.gg/Xj9nRKyKNV
[rust badge]: https://img.shields.io/badge/rust-1.95+-93450a.svg?style=for-the-badge&logo=rust
