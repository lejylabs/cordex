# Contributing to cordex

Thanks for helping! Bug reports, docs fixes and code are all welcome.

## Before you start

- **Questions and quick help**: ask on the [cordex Discord server](https://discord.gg/Xj9nRKyKNV).
- **Bugs**: open an issue with the bug template (version, what you did, what
  happened, logs with tokens removed).
- **Features or API changes**: open an issue first so we can agree on the
  design before you spend time on code.
- **Security problems**: never in a public issue, see [SECURITY.md](SECURITY.md).
- Small fixes (typos, docs, obvious bugs) can go straight to a pull request.

## Project layout

| Path | Content |
|---|---|
| `crates/model` | `cordex-model`: models and pure helpers (no async) |
| `crates/http` | `cordex-http`: REST client and rate limiter |
| `crates/gateway` | `cordex-gateway`: shards, shard manager, standby |
| `crates/interactions` | `cordex-interactions`: commands, responses, HTTP endpoint |
| `crates/cache` | `cordex-cache`: optional cache |
| `crates/facade` | `cordex`: facade, simple layer `Bot`, backfill, examples |
| `xtask` | `cargo xtask` quality gate, feature matrix, coverage tool |
| `docs/coverage.md` | generated API coverage report |

## Setup

- Rust toolchain from `rust-toolchain.toml` (installed automatically by
  `rustup`), plus [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny).
- Dependencies must be at least 3 days old on crates.io
  (`.cargo/config.toml`); builds use `--locked`.

```bash
cargo xtask check      # fmt, clippy, tests, docs, cargo-deny, layout rules
cargo xtask features   # every supported feature combination
cargo xtask coverage   # regenerate docs/coverage.md (needs curl, tar + network)
```

All three must pass before a pull request is merged; CI runs the same commands.
When you add or remove an endpoint, event or enum value, run
`cargo xtask coverage` and commit the updated `docs/coverage.md`. Every REST
method in `cordex-http` needs a doc comment with its route, e.g.
`` /// `GET /channels/{channel_id}` ``; the coverage tool reads these. New
model types for a docs table go into the `FIELDS` / `FLAGS` / `ENUMS` lists
in `xtask/src/coverage.rs`.

Each crate's `README.md` is also its rustdoc front page, so the examples in
it are compiled and run as doctests: keep them working. The root `README.md`
is copied to `crates/facade/README.md` (`cargo xtask layout-check` checks it).

For a live test against Discord, `crates/facade/examples/testbot.rs` reads
`DISCORD_TOKEN` from a `.env` file
(`cargo run -p cordex --example testbot --features cache`); it is not part of
the published crate.

## Rules for code

- **No panics on network input.** No `unwrap`/`expect` in library code; return
  errors instead. Long-running tasks stay behind the panic guard.
- **No `unsafe`.** `unsafe_code = "forbid"` in every crate.
- **Lints are never weakened.** An `#[allow(...)]` needs a `reason = "..."`.
- **Every public item has docs** (`missing_docs` is on).
- **Crate layering**: `cordex-model` has no async or network code; the
  allowed dependencies between crates are checked by `cargo xtask layout-check`.
- **No other Discord library** as a dependency, and no voice support.
- **Nothing outside the cache crate stores Discord data.**
- **New dependencies** need a short reason in the pull request; prefer what
  the workspace already uses.
- Tests for new behavior: mock tests (`cordex_http::mock`,
  `cordex_gateway::mock`) instead of live Discord calls.

## Pull requests

- One topic per pull request; keep the diff focused.
- Describe what changes and why; link the issue.
- Update `CHANGELOG.md` under "unreleased" for user-visible changes.
- Pull requests are squash-merged, so the PR title becomes the commit
  message: write it as a short imperative sentence ("Add …", "Fix …").
- Workflows from first-time and external contributors start after a
  maintainer approves them.

## License

By contributing you agree that your contribution is licensed under
`MIT OR Apache-2.0`, like the rest of the project, without additional terms.
