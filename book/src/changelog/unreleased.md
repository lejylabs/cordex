# Unreleased

Changes currently on `master` and scheduled for the next release.

- **Extended opt-in database type integrations (`cordex-model`):**
  - Added `Locale`, all `u64` bitflags (`Intents`, `UserFlags`, `MessageFlags`, `ChannelFlags`, `MemberFlags`, `RoleFlags`, `SystemChannelFlags`), and key `u8` enums (`ChannelType`, `InteractionType`, `PremiumTier`, `AutoModerationActionType`) across `postgres-types`, `rusqlite`, `redis`, and `mysql`.
  - Added native PostgreSQL `JSONB` / `JSON` `ToSql` / `FromSql` support for `Embed`, `Component`, and `AllowedMentions` (`postgres-types` feature).
- **Bot-layer interaction guards & collector enhancements (`cordex`):**
  - Added `require_guild()`, `require_nsfw()`, `require_role(role_id)`, `require_entitlement(sku_id)`, and `require_owner(&owners)` on `CommandContext`, `ComponentContext`, and `ModalContext` (with matching `Error::GuildOnly`, `Error::NsfwOnly`, `Error::MissingRole`, `Error::MissingEntitlement`, and `Error::OwnerOnly` variants).
  - Extended `ButtonClicks` with `.max(limit)`, `.filter(predicate)`, `.collected()`, and `.end_reason() -> Option<CollectorEndReason>` (`Timeout`, `Idle`, `Limit`, `Stopped`, `Closed`).
- **Redis `SessionStore` backend (`cordex-gateway`, `redis` feature):**
  - Added `RedisSessionStore` (configurable key prefix and TTL) for zero-downtime gateway `RESUME` across container and multi-node restarts; enabled automatically when `cordex` is built with `features = ["redis"]`.
