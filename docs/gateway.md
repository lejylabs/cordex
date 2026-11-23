# Gateway

`cordex-gateway` implements Discord Gateway v10 over WebSockets
(`tokio-websockets` + `rustls` with `aws-lc-rs` and system root certificates)
using JSON encoding and `zlib-stream` transport compression (`flate2` with
pure-Rust `zlib-rs`).

## Connection lifecycle

```text
Disconnected -> Connecting -> AwaitingHello -> Identifying -+
                                    |                       +-> Active <-> (dispatch)
                                    +-> Resuming -----------+
Active -> Reconnecting (resume | new session) | Disconnected (fatal) | ShuttingDown
```

- **Initial URL:** Fetched from `GET /gateway/bot` and connected with
  `?v=10&encoding=json&compress=zlib-stream`.
- **Resume:** Reconnects to the `resume_gateway_url` received in `READY`
  (never the original URL) with `session_id` and the last sequence number `s`.
- **Backoff:** Exponential backoff (1 s → 60 s, ±20 % jitter), reset after
  60 s of stable `Active` state.
- **Timeouts:** 20 s to receive `HELLO`, 30 s to receive `READY` / `RESUMED`.

## Heartbeats and zombie detection

- After `HELLO`, the first heartbeat is scheduled at
  `heartbeat_interval * random(0..1)`, then every `heartbeat_interval`.
- Heartbeats run inside the shard's own select loop and are never blocked by
  slow event consumers.
- If no `HEARTBEAT_ACK` arrives before the next heartbeat is due, the
  connection is treated as a zombie: the shard closes with code `4000` and
  resumes.
- Latency (send → ACK) is tracked per shard and exposed via `ShardStatus`.

## Close codes

- **Resumable** (`4000`, `4001`, `4002`, `4005`, `4008`, TCP drops): reconnect
  and send `RESUME`.
- **Session invalidated** (`1000`, `1001`, `4003`, `4007`, `4009`): reconnect
  with a fresh `IDENTIFY`.
- **Fatal** (`4004` auth failed, `4010` invalid shard, `4011` sharding
  required, `4012` invalid API version, `4013` invalid intents, `4014`
  disallowed intents): no reconnect loop. The shard transitions to
  `Disconnected` and emits a fatal `GatewayError` with a fix hint
  (`CloseCode::hint()`).

## Event filtering and compression

- **`EventTypeFlags`:** Envelopes (`{ op, d, s, t }`) are parsed in two
  steps. If `t` is not in the shard's `EventTypeFlags`, `d` is skipped without
  deserializing the payload (~8× faster than full parsing; see
  `docs/performance.md`).
- **Limits:** WebSocket frames default to a 16 MiB cap
  (`WsConnector::max_frame_bytes`); decompressed `zlib-stream` messages
  default to 16 MiB (`ShardBuilder::max_message_bytes`). Each new connection
  starts a fresh inflate context.

## Outbound rate limit

- Discord allows 120 gateway sends per 60 seconds per connection.
- `cordex-gateway` enforces a 120/60 s token bucket with 10 permits reserved
  for heartbeats (user commands may use at most 110/60 s).
- Payloads larger than 4096 bytes are rejected locally before sending.

## Backpressure and panic isolation

- Each shard delivers events into a bounded `tokio::sync::mpsc` channel
  (default capacity 1024).
- Slow consumer policies (`BackpressurePolicy`):
  - `Disconnect` *(default)*: closes resumable (`4000`) and resumes later so
    Discord replays missed events without unbounded memory growth.
  - `Block`: pauses reading while keeping heartbeats alive.
  - `DropOldest` / `DropNewest`: drops events and increments
    `ShardCounters::dropped_events`.
- **Panic guard:** Each connection run executes in its own task awaited by
  the shard supervisor. A panic becomes a non-fatal `GatewayError::Panic` and
  the shard reconnects; more than 5 panics in 10 minutes stops only that shard
  with `GatewayError::PanicLimit`.

## Graceful shutdown

- `CloseMode::Resumable` closes with code `4000` and returns `ResumeInfo` so
  another process can resume the session (rolling deploys).
- `CloseMode::Terminate` closes with code `1000` and ends the session.
