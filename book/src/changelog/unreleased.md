# Unreleased

Changes currently on `master` and scheduled for `0.2.1`.

- **New opt-in derive-macro crate `cordex-macros` (`macros` feature):**
  - Added `#[derive(CommandArgs)]` and `#[derive(CommandChoice)]` plus `CommandArgs` / `CommandChoice` traits, `Command::with_args::<A>()`, and `CommandOption::choices(...)` (strictly opt-in, off by default).
- **Opt-in PostgreSQL type integration (`postgres-types` feature on `cordex-model` & `cordex`):**
  - Added binary wire-format `ToSql` / `FromSql` for `Id<T>` (`INT8`), `Permissions` (`INT8`), `Timestamp` (`TIMESTAMPTZ`), and `ImageHash` (`TEXT` / `VARCHAR`) with offline roundtrip tests (no `sqlx`).
- **New Axum integration crate `cordex-axum`:**
  - Added `VerifiedInteraction` Ed25519 extractor (`401 Unauthorized` on invalid/missing signature), `interactions_router` (automatic `PING` → `PONG`), and `health_router` (`/healthz` & `/readyz` backed by `HealthSnapshot`).
