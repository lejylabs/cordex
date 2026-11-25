# cordex-migrate

Zero-dependency analyzer and safe mechanical migration CLI for moving **Serenity**, **Poise**, and **Twilight** Discord bots to [`cordex`](https://crates.io/crates/cordex).

## Install & Run

```bash
cargo install cordex-migrate
```

Or run directly from this repository:

```bash
cargo run -p cordex-migrate -- analyze path/to/your-bot
cargo run -p cordex-migrate -- apply path/to/your-bot          # dry-run (default)
cargo run -p cordex-migrate -- apply path/to/your-bot --write  # writes changes in place
```

## What it does

- **`analyze <path>`**: Scans `Cargo.toml` and `src/**/*.rs` for `serenity`, `poise`, `songbird`, and `twilight-*` usage, recommends the exact `cordex` dependency and minimal feature set, and prints a per-file breakdown of automatic rewrites vs. manual follow-ups.
- **`apply <path>`**:
  - Defaults to `--dry-run` so you can inspect every change before modifying files.
  - Performs deterministic, safe 1:1 rewrites (such as `twilight_*` module imports, `GatewayIntents::` -> `Intents::`, and common type paths).
  - Inserts idempotent `// TODO(cordex-migrate): ...` guidance comments above non-trivial patterns (`#[poise::command]`, `impl EventHandler`, `impl TypeMapKey`, Serenity builders, Twilight stream/cache setup) unless `--no-todos` is passed.

See the full [Migration Guide](https://lejylabs.github.io/cordex/migration.html) for side-by-side recipes and cheatsheets.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
