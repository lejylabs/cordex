# Unreleased

Changes currently on `master` and scheduled for `0.2.1`.

- **New opt-in derive-macro crate `cordex-macros` (`macros` feature):**
  - Added `#[derive(CommandArgs)]` and `#[derive(CommandChoice)]` plus `CommandArgs` / `CommandChoice` traits, `Command::with_args::<A>()`, and `CommandOption::choices(...)` (strictly opt-in, off by default).
- **Opt-in database & cache type integrations (`cordex-model` & `cordex`, all off by default):**
  - `postgres-types`: binary wire-format `ToSql` / `FromSql` for `Id<T>` (`INT8`), `Permissions` (`INT8`), `Timestamp` (`TIMESTAMPTZ`), and `ImageHash` (`TEXT` / `VARCHAR`).
  - `rusqlite` (SQLite): `ToSql` / `FromSql` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
  - `redis`: `ToRedisArgs` / `FromRedisValue` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
  - `mysql` (MySQL / MariaDB via `mysql_common`, works with both `mysql` and `mysql_async`): `FromValue` / `Into<Value>` for `Id<T>`, `Permissions`, `Timestamp`, and `ImageHash`.
- **New Axum integration crate `cordex-axum`:**
  - Added `VerifiedInteraction` Ed25519 extractor (`401 Unauthorized` on invalid/missing signature), `interactions_router` (automatic `PING` → `PONG`), and `health_router` (`/healthz` & `/readyz` backed by `HealthSnapshot`).
