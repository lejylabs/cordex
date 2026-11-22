//! In-process rate limiter and invalid-request guard (PLAN.md §6.5, §7).

use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::fmt;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use http::{HeaderMap, StatusCode};
use parking_lot::Mutex;
use tokio::sync::Notify;
use tokio::time::Instant;

use crate::error::HttpError;
use crate::route::Route;

const SHARDS: usize = 16;
const BUCKET_TTL: Duration = Duration::from_mins(10);
const INVALID_WINDOW: Duration = Duration::from_mins(10);
const DEFAULT_GLOBAL_RPS: u32 = 50;
pub(crate) const DEFAULT_WARN_AT: u32 = 8000;
pub(crate) const DEFAULT_REJECT_AT: u32 = 9500;

/// `X-RateLimit-Scope` values Discord sends on 429s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RateLimitScope {
    /// This bot / application.
    User,
    /// The global per-bot limit.
    Global,
    /// A shared resource (does **not** count toward the invalid-request guard).
    Shared,
    /// A value this version does not know yet.
    Unknown(Box<str>),
}

impl RateLimitScope {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "user" => Self::User,
            "global" => Self::Global,
            "shared" => Self::Shared,
            other => Self::Unknown(other.into()),
        }
    }
}

/// Pluggable rate limiter (PLAN.md §7.2). The default is [`InProcessRateLimiter`].
pub trait RateLimiter: Send + Sync + 'static {
    /// Wait until `route` may be sent. Decrements the bucket; call [`record`](Self::record)
    /// after a response or [`release`](Self::release) if no usable headers came back.
    fn acquire<'a>(
        &'a self,
        route: &'a Route,
    ) -> Pin<Box<dyn Future<Output = Result<(), HttpError>> + Send + 'a>>;

    /// Update bucket state from response headers (including 429s).
    fn record(&self, route: &Route, headers: &HeaderMap, status: StatusCode);

    /// Return a permit taken by [`acquire`](Self::acquire) when the request failed
    /// before headers (timeout, reset, transport error).
    fn release(&self, route: &Route);
}

/// Default in-process limiter: per-route buckets, global 50 rps, global 429 lock.
pub struct InProcessRateLimiter {
    shards: Box<[Shard]>,
    global_lock: GlobalLock,
    global_rps: GlobalRps,
    last_cleanup: Mutex<Instant>,
}

struct Shard {
    inner: Mutex<ShardInner>,
}

struct ShardInner {
    route_to_hash: HashMap<RouteKey, Box<str>>,
    unknown: HashMap<RouteKey, std::sync::Arc<Bucket>>,
    buckets: HashMap<BucketKey, std::sync::Arc<Bucket>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct RouteKey {
    method: http::Method,
    template: String,
    channel_id: Option<u64>,
    guild_id: Option<u64>,
    webhook_id: Option<u64>,
    webhook_token: Option<Box<str>>,
}

impl RouteKey {
    fn from_route(route: &Route) -> Self {
        Self {
            method: route.method.clone(),
            template: route.template.clone(),
            channel_id: route.major.channel_id,
            guild_id: route.major.guild_id,
            webhook_id: route.major.webhook_id,
            webhook_token: route.major.webhook_token().map(Box::from),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct BucketKey {
    hash: Box<str>,
    channel_id: Option<u64>,
    guild_id: Option<u64>,
    webhook_id: Option<u64>,
    webhook_token: Option<Box<str>>,
}

impl BucketKey {
    fn new(hash: Box<str>, key: &RouteKey) -> Self {
        Self {
            hash,
            channel_id: key.channel_id,
            guild_id: key.guild_id,
            webhook_id: key.webhook_id,
            webhook_token: key.webhook_token.clone(),
        }
    }
}

struct Bucket {
    state: Mutex<BucketState>,
    notify: Notify,
    last_used: Mutex<Instant>,
}

struct BucketState {
    remaining: i64,
    limit: i64,
    reset_at: Instant,
    window: Duration,
}

impl Bucket {
    fn probe() -> std::sync::Arc<Self> {
        let now = Instant::now();
        std::sync::Arc::new(Self {
            state: Mutex::new(BucketState {
                remaining: 1,
                limit: 1,
                // Far future: waiters sleep until `record` notifies.
                reset_at: now + Duration::from_secs(3600),
                window: Duration::from_secs(1),
            }),
            notify: Notify::new(),
            last_used: Mutex::new(now),
        })
    }

    fn known() -> std::sync::Arc<Self> {
        let now = Instant::now();
        std::sync::Arc::new(Self {
            state: Mutex::new(BucketState {
                remaining: 1,
                limit: 1,
                reset_at: now,
                window: Duration::from_secs(1),
            }),
            notify: Notify::new(),
            last_used: Mutex::new(now),
        })
    }

    fn touch(&self) {
        *self.last_used.lock() = Instant::now();
    }

    fn unused_since(&self, now: Instant) -> Duration {
        now.saturating_duration_since(*self.last_used.lock())
    }

    fn try_take(&self) -> Result<(), Duration> {
        self.touch();
        let mut st = self.state.lock();
        let now = Instant::now();
        if now >= st.reset_at {
            st.remaining = st.limit.max(0);
            st.reset_at = now + st.window;
        }
        if st.remaining > 0 {
            st.remaining -= 1;
            Ok(())
        } else {
            Err(st
                .reset_at
                .saturating_duration_since(now)
                .max(Duration::from_millis(1)))
        }
    }

    fn apply(&self, headers: &ParsedHeaders) {
        self.touch();
        let mut st = self.state.lock();
        if let Some(limit) = headers.limit {
            st.limit = i64::from(limit);
        }
        if let Some(remaining) = headers.remaining {
            st.remaining = i64::from(remaining);
        }
        if let Some(reset_after) = headers.reset_after {
            st.window = reset_after.max(Duration::from_millis(1));
            st.reset_at = Instant::now() + st.window;
        } else if headers.retry_after.is_some() && st.remaining <= 0 {
            let wait = headers.retry_after.unwrap_or(Duration::from_secs(1));
            st.window = wait.max(Duration::from_millis(1));
            st.reset_at = Instant::now() + st.window;
        }
        self.notify.notify_waiters();
    }

    fn release_one(&self) {
        let mut st = self.state.lock();
        st.remaining += 1;
        drop(st);
        self.notify.notify_waiters();
    }
}

struct GlobalLock {
    until: Mutex<Option<Instant>>,
    notify: Notify,
}

impl GlobalLock {
    fn new() -> Self {
        Self {
            until: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    async fn wait(&self) {
        loop {
            let delay = {
                let until = self.until.lock();
                match *until {
                    None => return,
                    Some(t) => {
                        let now = Instant::now();
                        if now >= t {
                            None
                        } else {
                            Some(t.saturating_duration_since(now))
                        }
                    }
                }
            };
            match delay {
                None => {
                    *self.until.lock() = None;
                    return;
                }
                Some(d) => {
                    let notified = self.notify.notified();
                    tokio::select! {
                        () = tokio::time::sleep(d) => {}
                        () = notified => {}
                    }
                }
            }
        }
    }

    fn set(&self, retry_after: Duration) {
        let until = Instant::now() + retry_after;
        {
            let mut slot = self.until.lock();
            match *slot {
                Some(existing) if existing >= until => {}
                _ => *slot = Some(until),
            }
        }
        self.notify.notify_waiters();
    }
}

struct GlobalRps {
    rate: f64,
    state: Mutex<(f64, Instant)>,
    notify: Notify,
}

impl GlobalRps {
    fn new(per_sec: u32) -> Self {
        let rate = f64::from(per_sec.max(1));
        Self {
            rate,
            state: Mutex::new((rate, Instant::now())),
            notify: Notify::new(),
        }
    }

    fn refill_locked(rate: f64, state: &mut (f64, Instant)) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.1).as_secs_f64();
        state.0 = (state.0 + elapsed * rate).min(rate);
        state.1 = now;
    }

    async fn take(&self) {
        loop {
            let wait = {
                let mut g = self.state.lock();
                Self::refill_locked(self.rate, &mut g);
                if g.0 >= 1.0 {
                    g.0 -= 1.0;
                    None
                } else {
                    let need = (1.0 - g.0) / self.rate;
                    Some(Duration::from_secs_f64(need.max(0.001)))
                }
            };
            match wait {
                None => return,
                Some(d) => {
                    let notified = self.notify.notified();
                    tokio::select! {
                        () = tokio::time::sleep(d) => {}
                        () = notified => {}
                    }
                }
            }
        }
    }

    fn give_back(&self) {
        let mut g = self.state.lock();
        Self::refill_locked(self.rate, &mut g);
        g.0 = (g.0 + 1.0).min(self.rate);
        drop(g);
        self.notify.notify_waiters();
    }
}

struct ParsedHeaders {
    limit: Option<u32>,
    remaining: Option<u32>,
    reset_after: Option<Duration>,
    retry_after: Option<Duration>,
    bucket: Option<Box<str>>,
    global: bool,
    scope: Option<RateLimitScope>,
}

impl ParsedHeaders {
    fn from_headers(headers: &HeaderMap, status: StatusCode) -> Self {
        let header_str = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
        };
        let parse_u32 = |name: &str| header_str(name).and_then(|s| s.parse().ok());
        let parse_dur = |name: &str| header_str(name).and_then(parse_secs);

        let global = header_str("x-ratelimit-global")
            .is_some_and(|v| v.eq_ignore_ascii_case("true"))
            || (status.as_u16() == 429
                && header_str("x-ratelimit-scope").is_some_and(|s| s == "global"));

        Self {
            limit: parse_u32("x-ratelimit-limit"),
            remaining: parse_u32("x-ratelimit-remaining"),
            reset_after: parse_dur("x-ratelimit-reset-after"),
            retry_after: parse_dur("retry-after"),
            bucket: header_str("x-ratelimit-bucket").map(Box::from),
            global,
            scope: header_str("x-ratelimit-scope").map(RateLimitScope::parse),
        }
    }
}

pub(crate) fn parse_secs(raw: &str) -> Option<Duration> {
    let v: f64 = raw.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(v.min(86_400.0)))
}

fn shard_index(key: &RouteKey) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    // Remainder is in `0..SHARDS`, so it always fits in `usize`.
    #[allow(clippy::cast_possible_truncation)]
    {
        hasher.finish() as usize % SHARDS
    }
}

impl InProcessRateLimiter {
    /// 50 requests/second global default.
    #[must_use]
    pub fn new() -> Self {
        Self::with_global_rps(DEFAULT_GLOBAL_RPS)
    }

    /// Global token bucket at `per_sec` (interaction routes skip it).
    #[must_use]
    pub fn with_global_rps(per_sec: u32) -> Self {
        let now = Instant::now();
        let shards = (0..SHARDS)
            .map(|_| Shard {
                inner: Mutex::new(ShardInner {
                    route_to_hash: HashMap::new(),
                    unknown: HashMap::new(),
                    buckets: HashMap::new(),
                }),
            })
            .collect();
        Self {
            shards,
            global_lock: GlobalLock::new(),
            global_rps: GlobalRps::new(per_sec),
            last_cleanup: Mutex::new(now),
        }
    }

    fn shard(&self, key: &RouteKey) -> &Shard {
        &self.shards[shard_index(key)]
    }

    fn maybe_cleanup_all(&self) {
        let now = Instant::now();
        {
            let mut last = self.last_cleanup.lock();
            if now.saturating_duration_since(*last) < BUCKET_TTL {
                return;
            }
            *last = now;
        }
        for shard in &self.shards {
            let mut inner = shard.inner.lock();
            inner
                .unknown
                .retain(|_, b| b.unused_since(now) < BUCKET_TTL);
            inner
                .buckets
                .retain(|_, b| b.unused_since(now) < BUCKET_TTL);
            let live: std::collections::HashSet<BucketKey> =
                inner.buckets.keys().cloned().collect();
            inner
                .route_to_hash
                .retain(|key, hash| live.contains(&BucketKey::new(hash.clone(), key)));
        }
    }

    fn bucket_for(&self, key: &RouteKey) -> std::sync::Arc<Bucket> {
        self.maybe_cleanup_all();
        let shard = self.shard(key);
        let mut inner = shard.inner.lock();
        if let Some(hash) = inner.route_to_hash.get(key) {
            let bkey = BucketKey::new(hash.clone(), key);
            inner
                .buckets
                .entry(bkey)
                .or_insert_with(Bucket::known)
                .clone()
        } else {
            inner
                .unknown
                .entry(key.clone())
                .or_insert_with(Bucket::probe)
                .clone()
        }
    }

    async fn acquire_inner(&self, route: &Route) -> Result<(), HttpError> {
        let key = RouteKey::from_route(route);
        self.global_lock.wait().await;
        if !route.global_exempt {
            self.global_rps.take().await;
        }
        loop {
            let bucket = self.bucket_for(&key);
            let notified = bucket.notify.notified();
            match bucket.try_take() {
                Ok(()) => return Ok(()),
                Err(wait) => {
                    tokio::select! {
                        () = tokio::time::sleep(wait) => {}
                        () = notified => {}
                    }
                }
            }
        }
    }

    fn record_inner(&self, route: &Route, headers: &HeaderMap, status: StatusCode) {
        let parsed = ParsedHeaders::from_headers(headers, status);
        if parsed.global || matches!(parsed.scope, Some(RateLimitScope::Global)) {
            let wait = parsed
                .retry_after
                .or(parsed.reset_after)
                .unwrap_or(Duration::from_secs(1));
            self.global_lock.set(wait);
        }

        let key = RouteKey::from_route(route);
        let bucket = if let Some(hash) = parsed.bucket.clone() {
            let shard = self.shard(&key);
            let mut inner = shard.inner.lock();
            inner.route_to_hash.insert(key.clone(), hash.clone());
            inner.unknown.remove(&key);
            let bkey = BucketKey::new(hash, &key);
            inner
                .buckets
                .entry(bkey)
                .or_insert_with(Bucket::known)
                .clone()
        } else {
            self.bucket_for(&key)
        };
        bucket.apply(&parsed);
    }

    fn release_inner(&self, route: &Route) {
        if !route.global_exempt {
            self.global_rps.give_back();
        }
        let key = RouteKey::from_route(route);
        self.bucket_for(&key).release_one();
    }

    #[cfg(test)]
    pub(crate) fn known_bucket_count(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.inner.lock().buckets.len())
            .sum()
    }

    #[cfg(test)]
    pub(crate) fn unknown_bucket_count(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.inner.lock().unknown.len())
            .sum()
    }
}

impl Default for InProcessRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter for InProcessRateLimiter {
    fn acquire<'a>(
        &'a self,
        route: &'a Route,
    ) -> Pin<Box<dyn Future<Output = Result<(), HttpError>> + Send + 'a>> {
        Box::pin(self.acquire_inner(route))
    }

    fn record(&self, route: &Route, headers: &HeaderMap, status: StatusCode) {
        self.record_inner(route, headers, status);
    }

    fn release(&self, route: &Route) {
        self.release_inner(route);
    }
}

impl fmt::Debug for InProcessRateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InProcessRateLimiter")
            .finish_non_exhaustive()
    }
}

/// Sliding 10-minute window of invalid responses (401, 403, 429 except shared).
pub(crate) struct InvalidRequestGuard {
    events: Mutex<VecDeque<Instant>>,
    warn_at: u32,
    reject_at: u32,
    warned: AtomicBool,
}

impl InvalidRequestGuard {
    pub(crate) fn new(warn_at: u32, reject_at: u32) -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
            warn_at,
            reject_at,
            warned: AtomicBool::new(false),
        }
    }

    fn prune_locked(events: &mut VecDeque<Instant>, now: Instant) {
        while events
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) > INVALID_WINDOW)
        {
            events.pop_front();
        }
    }

    pub(crate) fn check(&self) -> Result<(), HttpError> {
        let now = Instant::now();
        let mut events = self.events.lock();
        Self::prune_locked(&mut events, now);
        let count = u32::try_from(events.len()).unwrap_or(u32::MAX);
        if count >= self.reject_at {
            Err(HttpError::InvalidRequestLimit {
                count,
                threshold: self.reject_at,
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn record(&self) {
        let now = Instant::now();
        let mut events = self.events.lock();
        Self::prune_locked(&mut events, now);
        events.push_back(now);
        let count = u32::try_from(events.len()).unwrap_or(u32::MAX);
        if count >= self.warn_at
            && (count == self.warn_at
                || count.is_multiple_of(500)
                || !self.warned.swap(true, Ordering::Relaxed))
        {
            tracing::warn!(
                count,
                warn_at = self.warn_at,
                reject_at = self.reject_at,
                "invalid Discord requests approaching the Cloudflare ban window"
            );
        }
    }

    pub(crate) fn is_invalid(status: StatusCode, headers: &HeaderMap) -> bool {
        match status.as_u16() {
            401 | 403 => true,
            429 => {
                let scope = headers
                    .get("x-ratelimit-scope")
                    .and_then(|v| v.to_str().ok());
                scope != Some("shared")
            }
            _ => false,
        }
    }
}

/// Drop guard that returns a limiter permit unless `commit` is called.
pub(crate) struct PermitGuard<'a> {
    limiter: &'a dyn RateLimiter,
    route: &'a Route,
    pub(crate) committed: bool,
}

impl<'a> PermitGuard<'a> {
    pub(crate) fn new(limiter: &'a dyn RateLimiter, route: &'a Route) -> Self {
        Self {
            limiter,
            route,
            committed: false,
        }
    }
}

impl Drop for PermitGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.limiter.release(self.route);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use http::Method;

    fn route(path: &str, channel: u64) -> Route {
        let mut r = Route::raw(path, channel);
        r.method = Method::GET;
        r
    }

    fn headers(limit: u32, remaining: u32, reset_after: f64, bucket: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            "x-ratelimit-limit",
            HeaderValue::from_str(&limit.to_string()).unwrap(),
        );
        h.insert(
            "x-ratelimit-remaining",
            HeaderValue::from_str(&remaining.to_string()).unwrap(),
        );
        h.insert(
            "x-ratelimit-reset-after",
            HeaderValue::from_str(&reset_after.to_string()).unwrap(),
        );
        h.insert("x-ratelimit-bucket", HeaderValue::from_str(bucket).unwrap());
        h
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_bucket_serializes_probes() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        limiter.acquire(&a).await.unwrap();
        let wait = tokio::time::timeout(Duration::from_millis(5), limiter.acquire(&a)).await;
        assert!(
            wait.is_err(),
            "second probe must wait for the first response"
        );
        limiter.record(&a, &headers(5, 4, 1.0, "abc"), StatusCode::OK);
        limiter.acquire(&a).await.unwrap();
        assert_eq!(limiter.known_bucket_count(), 1);
        assert_eq!(limiter.unknown_bucket_count(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn shared_bucket_hash_is_reused() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}/x", 1);
        let b = route("/channels/{channel_id}/y", 1);
        limiter.acquire(&a).await.unwrap();
        limiter.record(&a, &headers(2, 1, 5.0, "same"), StatusCode::OK);
        limiter.acquire(&b).await.unwrap();
        limiter.record(&b, &headers(2, 0, 5.0, "same"), StatusCode::OK);
        assert_eq!(limiter.known_bucket_count(), 1);
        let blocked = tokio::time::timeout(Duration::from_millis(10), limiter.acquire(&a)).await;
        assert!(blocked.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn major_parameters_split_buckets() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        let b = route("/channels/{channel_id}", 2);
        limiter.acquire(&a).await.unwrap();
        limiter.record(&a, &headers(1, 0, 5.0, "h"), StatusCode::OK);
        limiter.acquire(&b).await.unwrap();
        limiter.record(&b, &headers(1, 0, 5.0, "h"), StatusCode::OK);
        assert_eq!(limiter.known_bucket_count(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn reset_after_unblocks_without_wall_clock() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        limiter.acquire(&a).await.unwrap();
        limiter.record(&a, &headers(1, 0, 1.0, "h"), StatusCode::OK);
        let start = Instant::now();
        limiter.acquire(&a).await.unwrap();
        assert!(start.elapsed() >= Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn missing_headers_release_permit() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        limiter.acquire(&a).await.unwrap();
        limiter.release(&a);
        limiter.acquire(&a).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn global_429_blocks_all_routes() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        let b = route("/guilds/{guild_id}", 9);
        limiter.acquire(&a).await.unwrap();
        let mut h = headers(5, 0, 2.0, "h");
        h.insert("x-ratelimit-global", HeaderValue::from_static("true"));
        h.insert("retry-after", HeaderValue::from_static("2"));
        limiter.record(&a, &h, StatusCode::TOO_MANY_REQUESTS);
        let start = Instant::now();
        limiter.acquire(&b).await.unwrap();
        assert!(start.elapsed() >= Duration::from_secs(2));
    }

    #[tokio::test(start_paused = true)]
    async fn interaction_routes_skip_global_rps() {
        let limiter = InProcessRateLimiter::with_global_rps(1);
        let mut interaction = Route::raw(
            "/interactions/{interaction_id}/{interaction_token}/callback",
            (1u64, "t"),
        );
        interaction.method = Method::POST;
        for _ in 0..5 {
            limiter.acquire(&interaction).await.unwrap();
            let mut h = HeaderMap::new();
            h.insert("x-ratelimit-limit", HeaderValue::from_static("100"));
            h.insert("x-ratelimit-remaining", HeaderValue::from_static("99"));
            h.insert("x-ratelimit-reset-after", HeaderValue::from_static("1"));
            h.insert("x-ratelimit-bucket", HeaderValue::from_static("i"));
            limiter.record(&interaction, &h, StatusCode::OK);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn unused_buckets_are_cleaned_after_ten_minutes() {
        let limiter = InProcessRateLimiter::new();
        let a = route("/channels/{channel_id}", 1);
        limiter.acquire(&a).await.unwrap();
        limiter.record(&a, &headers(5, 4, 1.0, "h"), StatusCode::OK);
        assert_eq!(limiter.known_bucket_count(), 1);
        tokio::time::advance(BUCKET_TTL + Duration::from_secs(1)).await;
        let b = route("/channels/{channel_id}", 2);
        limiter.acquire(&b).await.unwrap();
        assert_eq!(limiter.known_bucket_count(), 0);
        assert_eq!(limiter.unknown_bucket_count(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn invalid_guard_rejects_at_threshold() {
        let guard = InvalidRequestGuard::new(3, 5);
        for _ in 0..4 {
            guard.check().unwrap();
            guard.record();
        }
        guard.check().unwrap();
        guard.record();
        let err = guard.check().unwrap_err();
        assert!(matches!(
            err,
            HttpError::InvalidRequestLimit { count: 5, .. }
        ));
    }

    #[test]
    fn shared_429_is_not_invalid() {
        let mut h = HeaderMap::new();
        h.insert("x-ratelimit-scope", HeaderValue::from_static("shared"));
        assert!(!InvalidRequestGuard::is_invalid(
            StatusCode::TOO_MANY_REQUESTS,
            &h
        ));
        assert!(InvalidRequestGuard::is_invalid(
            StatusCode::TOO_MANY_REQUESTS,
            &HeaderMap::new()
        ));
        assert!(InvalidRequestGuard::is_invalid(
            StatusCode::UNAUTHORIZED,
            &HeaderMap::new()
        ));
    }
}
