# Zeny Discord – Rust Library
## Project Plan & Technical Specification

**Project:** Independent Discord API Library  
**Language:** Rust  
**Runtime:** Tokio  
**Target:** High performance, low memory usage, low latency, complete control over Discord API integration  
**Development target:** 5–10 days for initial production-ready Zeny scope  
**Voice Support:** Explicitly out of scope

---

# 1. Project Goals

Develop an independent, modular Discord API library in Rust without depending on existing Discord libraries such as Twilight, Serenity or Poise.

The library should provide all functionality required by ZenyBot while remaining reusable for future projects.

### Primary goals

- Full control over Discord Gateway implementation.
- Full control over Discord REST API implementation.
- Manual and automatic sharding support.
- Efficient memory usage and minimal allocations.
- Low latency and high throughput.
- Minimal abstraction overhead.
- Asynchronous architecture using Tokio.
- Current Discord API support.
- Easy integration into existing Rust applications.
- Modular architecture with independently usable components.
- Long-term maintainability without relying on third-party Discord libraries.

### Non-goals

- Voice connections.
- Audio streaming.
- Songbird integration.
- Opus encoding/decoding.
- UDP voice transport.
- DAVE voice encryption.
- Discord user account automation.
- Unofficial Discord API functionality.

**Important:** The library is designed for official Discord bot applications.

---

# 2. Technology Stack

| Component | Technology |
|---|---|
| Language | Rust (stable) |
| Async Runtime | Tokio |
| WebSocket | tokio-websockets |
| HTTP Client | Hyper |
| TLS | rustls |
| Serialization | Serde |
| JSON | serde_json |
| Compression | flate2 / zlib |
| Error Handling | thiserror |
| Logging | tracing |
| Cryptography | ed25519-dalek |
| Testing | tokio-test / integration tests |

Dependencies should be kept minimal.

Avoid unnecessary abstractions and dependencies that introduce additional runtime overhead.

Use efficient data structures and avoid cloning large payloads unnecessarily.

---

# 3. Project Architecture

Use a Cargo workspace with independently usable crates.

```text
zeny-discord/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── crates/
│   ├── discord/
│   │   └── Main public API and client
│   │
│   ├── discord-models/
│   │   └── Discord API models and types
│   │
│   ├── discord-http/
│   │   └── REST API implementation
│   │
│   ├── discord-gateway/
│   │   └── Gateway connection and event handling
│   │
│   ├── discord-cache/
│   │   └── Optional in-memory cache
│   │
│   └── discord-interactions/
│       └── Interaction types and helpers
│
├── tests/
│   ├── gateway/
│   ├── http/
│   ├── sharding/
│   └── integration/
│
├── examples/
│   ├── basic_bot.rs
│   ├── gateway.rs
│   ├── manual_sharding.rs
│   ├── automatic_sharding.rs
│   └── interactions.rs
│
└── docs/
    ├── architecture.md
    ├── gateway.md
    ├── sharding.md
    └── performance.md
```

All components should be independently usable wherever practical.

The main `discord` crate should provide a convenient unified interface.

---

# 4. Discord Models

Implement the Discord API models required by the supported REST endpoints and Gateway events.

### Required models

- User
- Guild
- GuildMember
- Role
- Channel
- Message
- MessageAttachment
- MessageEmbed
- MessageReaction
- Emoji
- Sticker
- Invite
- Webhook
- Application
- ApplicationCommand
- Interaction
- InteractionResponse
- Component
- Modal
- AutoModerationRule
- AuditLog
- ScheduledEvent
- Entitlement
- SKU
- Subscription
- Poll
- SoundboardSound
- VoiceState (data model only, no voice transport)

### Requirements

- Correct serialization and deserialization.
- Optional fields handled correctly.
- Unknown JSON fields must not break deserialization.
- Correct handling of Discord Snowflakes.
- Bitflags for permissions and relevant flag fields.
- Efficient cloning and ownership handling.
- Support for partial objects where Discord returns incomplete resources.
- Raw JSON access for unsupported or newly introduced fields.

### API generation

Use the official Discord API specification as a starting point for REST models.

Do not blindly trust generated code.

Generated models must be reviewed and tested against the official documentation.

Gateway payloads and event models must be handled separately because the REST OpenAPI specification does not describe the Gateway protocol.

---

# 5. REST API Implementation

Implement a centralized HTTP client.

### Core functionality

- Bot token authentication.
- Authorization headers.
- Correct Discord User-Agent.
- JSON request/response handling.
- Multipart requests and file uploads.
- HTTP error handling.
- Request timeouts.
- Connection pooling.
- TLS support.
- Rate limit management.
- Retry handling where safe.
- Raw endpoint support.

### Endpoint coverage

Implement the official Discord REST resources:

- Applications
- Application Commands
- Audit Logs
- Auto Moderation
- Channels
- Emojis
- Entitlements
- Guilds
- Guild Scheduled Events
- Guild Templates
- Interactions
- Invites
- Messages
- Polls
- Roles
- SKUs
- Soundboard
- Stage Instances
- Stickers
- Subscriptions
- Users
- Webhooks

### Rate Limiting

Implement a centralized rate limiter.

Requirements:

- Route bucket identification.
- Bucket hash handling.
- Global rate limits.
- Per-route limits.
- Shared rate limits.
- Retry-After handling.
- X-RateLimit headers.
- Dynamic bucket discovery.
- Concurrent request coordination.
- Avoid unnecessary requests when a bucket is exhausted.

Rate limiting must be thread-safe and work across concurrent asynchronous requests.

---

# 6. Gateway Implementation

Implement the Discord Gateway protocol independently.

This is one of the most important components of the library.

### Connection management

- Gateway URL discovery.
- WebSocket connection.
- TLS.
- Gateway version selection.
- JSON encoding.
- Gateway payload parsing.
- Compression support.
- Connection lifecycle management.

### Gateway protocol

Implement all relevant Gateway opcodes:

- DISPATCH
- HEARTBEAT
- IDENTIFY
- PRESENCE_UPDATE
- VOICE_STATE_UPDATE (Gateway payload only)
- RESUME
- RECONNECT
- REQUEST_GUILD_MEMBERS
- INVALID_SESSION
- HELLO
- HEARTBEAT_ACK

### Connection reliability

- Heartbeat scheduling.
- Heartbeat ACK tracking.
- Sequence number tracking.
- Session ID management.
- Resume support.
- Automatic reconnect.
- Invalid session handling.
- Exponential backoff with sensible limits.
- Connection timeout handling.
- Graceful shutdown.

### Gateway events

Support the complete set of currently documented Gateway events.

Include:

- Ready
- Resumed
- Guild events
- Channel events
- Message events
- Reaction events
- Member events
- Role events
- Presence events
- Typing events
- Interaction events
- Application events
- Auto Moderation events
- Scheduled Event events
- Entitlement events
- Subscription events
- Poll events
- Soundboard events

Unknown or unsupported events must remain accessible through raw event payloads.

Never crash simply because Discord introduces a new event.

### Intents

Implement all currently supported Gateway intents.

Support:

- Intent bitflags.
- Required intent configuration.
- Privileged intents.
- Configurable intent selection.

---

# 7. Sharding System

**Both manual and automatic sharding are mandatory requirements.**

Sharding must be designed into the Gateway architecture from the beginning.

## 7.1 Manual Sharding

Allow users to configure their own shard configuration.

Required configuration:

- `shard_id`
- `shard_count`

Example:

```rust
let shard = ShardBuilder::new(token)
    .shard_id(0)
    .shard_count(4)
    .intents(intents)
    .build()
    .await?;
```

Requirements:

- Explicit shard ID.
- Explicit total shard count.
- Independent shard lifecycle.
- Independent Gateway connection.
- Individual reconnect and resume.
- Independent event processing.
- Support for running shards in separate tasks or processes.

## 7.2 Automatic Sharding

Provide an automatic Shard Manager.

The manager should:

1. Request the recommended shard count from Discord's `/gateway/bot` endpoint.
2. Retrieve session start limit information.
3. Read `max_concurrency`.
4. Create the required shard instances.
5. Assign shard IDs automatically.
6. Respect Discord Identify concurrency limits.
7. Start shards in permitted Identify windows.
8. Monitor shard states.
9. Handle reconnects and restarts.
10. Shut down shards gracefully.

Example:

```rust
let manager = ShardManager::builder(token)
    .intents(intents)
    .automatic_sharding()
    .build()
    .await?;

manager.start().await?;
```

## 7.3 Shard Manager

The Shard Manager should provide:

- Start all shards.
- Stop all shards.
- Restart individual shards.
- Restart all shards.
- Shard status monitoring.
- Identify concurrency coordination.
- Session start limit tracking.
- Per-shard error handling.
- Per-shard event streams.
- Configurable startup behavior.

### Important distinction

Automatic sharding means automatically selecting the recommended shard count and managing shard startup.

It does not mean arbitrarily changing the shard count while existing Gateway sessions remain active.

Changing the shard count requires an appropriately coordinated restart.

---

# 8. Interactions

Implement the Discord Interaction system.

### Application Commands

- Slash Commands.
- User Commands.
- Message Commands.
- Command registration.
- Command editing.
- Command deletion.
- Global Commands.
- Guild Commands.
- Command permissions.
- Autocomplete.

### Components

- Buttons.
- Select Menus.
- String Select.
- User Select.
- Role Select.
- Channel Select.
- Mentionable Select.

### Modals

- Modal creation.
- Text Inputs.
- Modal submission parsing.
- Interaction response handling.

### Interaction handling

- Interaction IDs.
- Interaction tokens.
- Response types.
- Deferred responses.
- Follow-up messages.
- Ephemeral responses.
- Response editing.
- Interaction acknowledgement.

Support both Gateway-delivered interactions and HTTP interaction endpoints where applicable.

---

# 9. Cache

The cache must be optional.

Users should be able to use the library without maintaining a complete in-memory representation of their Discord data.

### Supported cache resources

- Guilds.
- Channels.
- Users.
- Members.
- Roles.
- Messages.
- Emojis.
- Stickers.
- Voice States.

### Requirements

- Configurable cache components.
- Selective caching.
- Efficient lookup.
- Event-driven updates.
- Correct cache invalidation.
- Thread-safe access.
- Avoid unnecessary allocations.
- Configurable cache limits where appropriate.

Do not force users to enable caching.

---

# 10. Public API Design

The public API should be straightforward and predictable.

Example:

```rust
use zeny_discord::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder("BOT_TOKEN")
        .intents(GatewayIntents::all())
        .build()
        .await?;

    client.start().await?;

    Ok(())
}
```

The API should support:

- Simple client creation.
- Custom HTTP configuration.
- Gateway configuration.
- Manual sharding.
- Automatic sharding.
- Optional cache.
- Event handlers.
- Raw event handlers.
- REST access.
- Interaction helpers.

Avoid unnecessary macros and complex abstractions.

The public API should remain flexible enough to allow advanced users to access lower-level components.

---

# 11. Performance Requirements

Performance is a primary design objective.

### Memory

- Avoid unnecessary allocations.
- Avoid excessive cloning.
- Use efficient collections.
- Make caching optional.
- Avoid keeping unnecessary Gateway payloads.
- Reuse buffers where practical.

### Latency

- Minimize event processing overhead.
- Avoid unnecessary synchronization.
- Avoid blocking operations.
- Keep Gateway event dispatch efficient.
- Avoid unnecessary serialization/deserialization passes.

### Throughput

- Support concurrent REST requests.
- Efficient Gateway event processing.
- Independent shard processing.
- Efficient rate limit coordination.
- Avoid global locks where they become bottlenecks.

### Benchmarking

Benchmark at least:

- JSON deserialization.
- Gateway payload parsing.
- Event dispatch.
- REST request overhead.
- Cache lookup.
- Memory consumption under event load.

Do not optimize based on assumptions alone.

---

# 12. Testing

Testing is mandatory, especially for Gateway and sharding.

### Unit Tests

- Model serialization.
- Model deserialization.
- Snowflake handling.
- Permission flags.
- Gateway opcode parsing.
- Event parsing.
- Rate limit calculations.
- Cache operations.

### Integration Tests

- Gateway connection.
- Identify.
- Heartbeats.
- Heartbeat ACK.
- Resume.
- Reconnect.
- Invalid sessions.
- Manual sharding.
- Automatic sharding.
- Identify concurrency.
- REST authentication.
- Rate limiting.
- Multipart requests.

### Reliability Tests

- Unexpected WebSocket closure.
- Network interruptions.
- Invalid payloads.
- Unknown Gateway events.
- Discord API errors.
- Concurrent REST requests.
- Shard restart.
- Graceful shutdown.

Use mocked Discord responses where appropriate.

Also run real integration tests against a dedicated Discord test bot.

---

# 13. Documentation and API Maintenance

The library must remain independent of the update speed of other Discord libraries.

Use official sources:

- https://docs.discord.com/developers
- https://github.com/discord/discord-api-docs
- https://github.com/discord/discord-api-spec

### Maintenance workflow

Create an automated API update workflow.

It should:

1. Regularly check the official Discord documentation repository.
2. Detect changes in REST resources.
3. Detect new Gateway events.
4. Detect changed payload structures.
5. Identify new fields and deprecated fields.
6. Compare the official OpenAPI specification.
7. Generate or update Rust models where appropriate.
8. Generate tests for changed models.
9. Produce a report of potentially breaking changes.

The OpenAPI specification is a public preview and must not be treated as the sole authoritative production source.

When documentation and generated specifications disagree, follow the official Developer Documentation.

---

# 14. Development Roadmap

## Phase 1 – Foundation

- [ ] Initialize Cargo workspace.
- [ ] Create all crates.
- [ ] Configure shared dependencies.
- [ ] Establish public API structure.
- [ ] Implement error handling.
- [ ] Configure tracing.
- [ ] Establish coding conventions.

## Phase 2 – Models

- [ ] Implement Snowflake.
- [ ] Implement common Discord models.
- [ ] Implement permissions and flags.
- [ ] Implement Gateway payload structures.
- [ ] Implement raw JSON fallback.
- [ ] Validate serialization and deserialization.

## Phase 3 – REST

- [ ] Implement HTTP client.
- [ ] Implement authentication.
- [ ] Implement User-Agent.
- [ ] Implement request/response handling.
- [ ] Implement rate limiter.
- [ ] Implement core REST endpoints.
- [ ] Implement multipart uploads.
- [ ] Implement raw endpoint support.

## Phase 4 – Gateway

- [ ] Implement WebSocket transport.
- [ ] Implement Gateway discovery.
- [ ] Implement HELLO.
- [ ] Implement IDENTIFY.
- [ ] Implement heartbeats.
- [ ] Implement heartbeat ACK.
- [ ] Implement DISPATCH.
- [ ] Implement sequence tracking.
- [ ] Implement session management.
- [ ] Implement RESUME.
- [ ] Implement reconnect handling.
- [ ] Implement all documented events.
- [ ] Implement raw event support.

## Phase 5 – Sharding

- [ ] Implement manual sharding.
- [ ] Implement `/gateway/bot` discovery.
- [ ] Implement recommended shard count.
- [ ] Implement session start limit tracking.
- [ ] Implement Identify concurrency.
- [ ] Implement automatic Shard Manager.
- [ ] Implement individual shard lifecycle.
- [ ] Implement monitoring.
- [ ] Test multiple shards.

## Phase 6 – Interactions

- [ ] Implement Application Commands.
- [ ] Implement command registration.
- [ ] Implement Buttons.
- [ ] Implement Select Menus.
- [ ] Implement Modals.
- [ ] Implement Autocomplete.
- [ ] Implement interaction responses.
- [ ] Implement follow-up messages.

## Phase 7 – Cache

- [ ] Implement optional cache.
- [ ] Implement selective caching.
- [ ] Implement event-driven cache updates.
- [ ] Implement cache invalidation.
- [ ] Test concurrent access.

## Phase 8 – Integration and Release

- [ ] Create basic bot example.
- [ ] Create manual sharding example.
- [ ] Create automatic sharding example.
- [ ] Integrate with ZenyBot.
- [ ] Run integration tests.
- [ ] Check memory usage.
- [ ] Benchmark critical paths.
- [ ] Fix critical bugs.
- [ ] Write README.
- [ ] Prepare version 0.1.0.

---

# 15. Time Target

Target: 5–10 days of intensive AI-assisted development.

| Day | Focus |
|---|---|
| 1 | Workspace, architecture, models |
| 2 | REST client, authentication, rate limits |
| 3 | REST endpoints and model coverage |
| 4 | Gateway connection and protocol |
| 5 | Heartbeats, resume, reconnect |
| 6 | Gateway events and sharding |
| 7 | Automatic Shard Manager and interactions |
| 8 | Cache and Zeny integration |
| 9 | Integration tests and bug fixes |
| 10 | Performance, documentation and release |

This schedule is an ambitious target, not a guarantee of complete Discord API coverage.

Prioritize a working, reliable foundation and all functionality required by Zeny.

Additional REST endpoints and models can be added incrementally.

---

# 16. Definition of Done – Version 0.1.0

The initial release should satisfy these requirements:

- [ ] Independent Rust Discord library.
- [ ] No dependency on Twilight, Serenity or Poise.
- [ ] No voice implementation.
- [ ] Functional REST client.
- [ ] Centralized rate limiting.
- [ ] Functional Gateway connection.
- [ ] Heartbeats and ACK handling.
- [ ] Session management.
- [ ] Resume and reconnect.
- [ ] Gateway event handling.
- [ ] Raw event fallback.
- [ ] Manual sharding.
- [ ] Automatic sharding.
- [ ] Identify concurrency handling.
- [ ] Interaction support.
- [ ] Optional cache.
- [ ] Working ZenyBot integration.
- [ ] Basic automated test coverage.
- [ ] Documented public API.

---

# 17. Important Development Rules

1. Do not introduce dependencies on existing Discord libraries.
2. Do not implement voice functionality.
3. Do not overengineer the initial architecture.
4. Prefer simple and efficient Rust implementations.
5. Keep modules independently usable.
6. Keep raw API access available.
7. Never assume Discord payloads will remain unchanged.
8. Unknown events must not crash the client.
9. Handle rate limits correctly.
10. Respect Discord Gateway session and Identify limits.
11. Do not claim API coverage without verifying it.
12. Do not claim tests or benchmarks were executed unless they actually were.
13. Prioritize correctness over premature optimization.
14. Keep the library independent from ZenyBot-specific business logic.

**Final objective:** Build a lightweight, high-performance and independently maintained Discord library in Rust that gives Zeny full control over its Discord integration and can serve as a foundation for future projects.