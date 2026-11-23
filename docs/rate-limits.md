# Rate Limits

`cordex-http` implements Discord's per-route and global rate limits plus an
invalid-request guard to prevent Cloudflare IP bans.

## How Discord rate limits work

- **Per-route buckets:** Identified by `X-RateLimit-Bucket` (an opaque hash)
  combined with the route's **major parameters** (`channel_id`, `guild_id`,
  `webhook_id` + `webhook_token`). Multiple routes can share one bucket.
- **Timing:** `cordex-http` uses only relative durations (`X-RateLimit-Reset-After`
  and `retry_after` in 429 bodies) against `tokio::time::Instant`, avoiding
  wall-clock drift issues.
- **Global limit:** 50 requests/second per bot (configurable via
  `ClientBuilder::global_rps`). Interaction callback routes
  (`/interactions/{id}/{token}/callback`) and interaction webhook follow-ups
  (marked with `Route::interaction()`) are exempt from the global limit.

## In-process limiter (`InProcessRateLimiter`)

- **Unknown buckets:** Until a route's bucket hash is learned from the first
  response, concurrency for that route key is capped at 1 so parallel startup
  requests cannot trigger a burst of 429s.
- **Locking:** 16 `parking_lot::Mutex` shards; locks are never held across
  `.await`. Waiters sleep until `reset_at` or a `Notify` from `record`.
- **Cleanup:** Buckets unused for 10 minutes are pruned lazily on acquire.
- **Pluggable:** Implement `trait RateLimiter` (`acquire`, `record`,
  `release`, `bucket_count`) to share rate-limit state across multiple
  processes (e.g. via Redis/Dragonfly).

## Retry policy

| Response | Behavior |
|---|---|
| `429` | Wait `retry_after` and retry (max. 3 retries) if wait ≤ cap (default 10 s); otherwise return `HttpError::RateLimited` immediately. |
| `502` / `503` / `504` or pre-response reset | Retry idempotent methods (`GET`, `HEAD`, `PUT`, `DELETE`) only, with exponential backoff + jitter (100 ms → 1.6 s, max. 3 retries). |
| `500`, `400`, `403`, `404` | Never retried; returned as `HttpError::Discord`. |
| `401` | Never retried; poisons the client so subsequent requests fail immediately with `HttpError::Unauthorized`. |

## Invalid-request guard (Cloudflare protection)

Discord bans IPs that send more than 10,000 invalid requests (`401`, `403`, or
non-shared `429`) within 10 minutes.

- `Client` tracks invalid responses in a sliding 10-minute window.
- At 8,000 it logs a `warn`; at 9,500 (configurable) it rejects new requests
  locally with `HttpError::InvalidRequestLimit` until the window has room.
- Shared-scope 429s (`X-RateLimit-Scope: shared`) are excluded as documented
  by Discord.
- Current counts and 429s by scope are exposed via `Client::metrics()`.
