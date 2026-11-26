//! Lightweight thread-safe `Arc<str>` string interner for `cordex-cache`.
//!
//! Deduplicates recurring strings (locales, guild features, role/channel names)
//! so identical strings share a single heap allocation.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::RwLock;

/// Thread-safe `Arc<str>` interner.
#[derive(Debug, Default)]
pub struct StringInterner {
    pool: RwLock<HashSet<Arc<str>>>,
}

impl StringInterner {
    /// Create an empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a shared [`Arc<str>`] for `s`, allocating only on first sight.
    pub fn intern(&self, s: &str) -> Arc<str> {
        if let Some(existing) = self.pool.read().get(s) {
            return Arc::clone(existing);
        }
        let mut pool = self.pool.write();
        if let Some(existing) = pool.get(s) {
            return Arc::clone(existing);
        }
        let arc: Arc<str> = Arc::from(s);
        pool.insert(Arc::clone(&arc));
        arc
    }

    /// Number of distinct interned strings.
    pub fn len(&self) -> usize {
        self.pool.read().len()
    }

    /// Whether no strings are interned.
    pub fn is_empty(&self) -> bool {
        self.pool.read().is_empty()
    }

    /// Drop strings that are no longer referenced anywhere outside the interner
    /// (`Arc::strong_count == 1`) and release excess map capacity.
    pub fn prune_unused(&self) -> usize {
        let mut pool = self.pool.write();
        let before = pool.len();
        pool.retain(|s| Arc::strong_count(s) > 1);
        pool.shrink_to_fit();
        before - pool.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_deduplicates_and_prunes_unused() {
        let interner = StringInterner::new();
        let a = interner.intern("en-US");
        let b = interner.intern("en-US");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(interner.len(), 1);

        drop(a);
        assert_eq!(interner.prune_unused(), 0);
        drop(b);
        assert_eq!(interner.prune_unused(), 1);
        assert!(interner.is_empty());
    }
}
