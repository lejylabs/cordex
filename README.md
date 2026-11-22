# discord-rs

Independent Discord bot API library for Rust (Tokio). No voice. Working name
`drs`; the final project name is not decided yet.

Status: Phase 0 (workspace skeleton). Plan and specification: [PLAN.md](PLAN.md).

## Crates

| Crate | Content |
|---|---|
| `crates/facade` | facade, re-exports behind features |
| `crates/model` | models, snowflakes, flags, gateway payloads |
| `crates/http` | REST client, rate limiter |
| `crates/gateway` | shards, shard manager |
| `crates/cache` | optional cache |
| `crates/interactions` | interactions, HTTP endpoint |

## Develop

Toolchain pinned in `rust-toolchain.toml`; dependencies need 3 days of publish
age (`.cargo/config.toml`); builds use `--locked`.

```bash
cargo fmt --all
cargo xtask check          # fmt, clippy, tests, docs, cargo-deny, layout-check
cargo xtask layout-check   # crate dependency rules only
```
