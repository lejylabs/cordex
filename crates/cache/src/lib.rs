#![doc = include_str!("../README.md")]

mod cache;
mod config;
mod intern;
mod strip;

pub use cache::{CacheStats, InMemoryCache, PermissionCacheError, PreviousState};
pub use config::{CacheConfig, CacheParts, CachePolicy, ConfigWarning, ResourceType};
pub use intern::StringInterner;

#[cfg(test)]
mod tests;
