//! Optional in-memory cache, fed with gateway events.
//!
//! **Everything is off by default.** [`CacheConfig::default`] caches
//! nothing; every resource ([`ResourceType`]), every optional part of a
//! resource ([`CacheParts`]) and every limit is its own switch, also at
//! runtime ([`InMemoryCache::set_config`]). The cache has no network access
//! and never fills itself: call [`InMemoryCache::update`] with each event.
//!
//! ```
//! use cordex_cache::{CacheConfig, CacheParts, InMemoryCache, ResourceType};
//!
//! let cache = InMemoryCache::new(
//!     CacheConfig::new()
//!         .resources(ResourceType::GUILDS | ResourceType::MEMBERS | ResourceType::USERS)
//!         .parts(CacheParts::MEMBER_ROLES),
//! );
//! // for each gateway event: cache.update(&event);
//! # let _ = cache;
//! ```

mod cache;
mod config;
mod strip;

pub use cache::{CacheStats, InMemoryCache, PermissionCacheError};
pub use config::{CacheConfig, CacheParts, ConfigWarning, ResourceType};

#[cfg(test)]
mod tests;
