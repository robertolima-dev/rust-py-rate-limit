//! Fixed Window rate limiting algorithm.
//!
//! Each key keeps a counter and a window start time. Within a window of
//! `window_seconds`, up to `limit` requests are admitted; further requests are
//! rejected until the window rolls over, at which point the counter resets.
//!
//! ## Concurrency
//!
//! State lives in a [`DashMap`], a sharded concurrent hash map. Each key maps
//! to one shard guarded by its own lock, so operations on *different* keys
//! proceed in parallel — there is no global lock on the critical path. While we
//! hold a per-key entry guard we never touch the map again, so we cannot
//! deadlock against ourselves.

use dashmap::DashMap;

use crate::entry::RateLimitEntry;

/// The result of evaluating a request against the limiter.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub allowed: bool,
    pub limit: u64,
    pub remaining: u64,
    pub reset_after_seconds: u64,
    pub retry_after_seconds: u64,
}

/// Fixed Window limiter state, independent of any Python types.
pub struct FixedWindow {
    pub limit: u64,
    pub window_seconds: u64,
    entries: DashMap<String, RateLimitEntry>,
}

impl FixedWindow {
    pub fn new(limit: u64, window_seconds: u64) -> Self {
        Self {
            limit,
            window_seconds,
            entries: DashMap::new(),
        }
    }

    /// Evaluates `key` at time `now`, admitting the request if it fits within
    /// the current window. This is the single mutating entry point shared by
    /// `allow` and `check`.
    pub fn consume(&self, key: &str, now: u64) -> CheckOutcome {
        // `entry(...).or_insert_with(...)` takes a write guard on the key's
        // shard. `to_string()` only allocates on the (rare) insert path in
        // practice; DashMap requires an owned key to insert.
        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(|| RateLimitEntry::new(now));

        // Roll the window over if it has expired.
        if entry.is_expired(now, self.window_seconds) {
            entry.window_start = now;
            entry.count = 0;
        }

        let elapsed = now.saturating_sub(entry.window_start);
        let reset_after = self.window_seconds.saturating_sub(elapsed);

        let allowed = entry.count < self.limit;
        if allowed {
            entry.count += 1;
        }
        let remaining = self.limit.saturating_sub(entry.count);

        CheckOutcome {
            allowed,
            limit: self.limit,
            remaining,
            reset_after_seconds: reset_after,
            retry_after_seconds: if allowed { 0 } else { reset_after },
        }
    }

    /// Returns how many requests remain in `key`'s current window without
    /// consuming one. An unknown or expired key has the full limit available.
    pub fn remaining(&self, key: &str, now: u64) -> u64 {
        match self.entries.get(key) {
            Some(entry) if !entry.is_expired(now, self.window_seconds) => {
                self.limit.saturating_sub(entry.count)
            }
            _ => self.limit,
        }
    }

    /// Removes `key`'s state. Returns `true` if the key existed.
    pub fn reset(&self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Removes all keys.
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Removes every key whose window has expired. Returns the number removed.
    pub fn cleanup_expired(&self, now: u64) -> usize {
        let mut removed = 0usize;
        self.entries.retain(|_key, entry| {
            let expired = entry.is_expired(now, self.window_seconds);
            if expired {
                removed += 1;
            }
            !expired
        });
        removed
    }

    /// Number of keys currently tracked (including not-yet-cleaned expired
    /// keys).
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_up_to_limit_then_blocks() {
        let fw = FixedWindow::new(2, 60);
        assert!(fw.consume("k", 1_000).allowed);
        assert!(fw.consume("k", 1_000).allowed);
        assert!(!fw.consume("k", 1_000).allowed);
    }

    #[test]
    fn resets_after_window() {
        let fw = FixedWindow::new(1, 10);
        assert!(fw.consume("k", 1_000).allowed);
        assert!(!fw.consume("k", 1_005).allowed);
        // 10s later the window has rolled over.
        assert!(fw.consume("k", 1_010).allowed);
    }

    #[test]
    fn remaining_reflects_consumption() {
        let fw = FixedWindow::new(3, 60);
        assert_eq!(fw.remaining("k", 1_000), 3);
        fw.consume("k", 1_000);
        assert_eq!(fw.remaining("k", 1_000), 2);
    }

    #[test]
    fn keys_are_independent() {
        let fw = FixedWindow::new(1, 60);
        assert!(fw.consume("a", 1_000).allowed);
        assert!(fw.consume("b", 1_000).allowed);
        assert!(!fw.consume("a", 1_000).allowed);
    }

    #[test]
    fn cleanup_removes_expired() {
        let fw = FixedWindow::new(1, 10);
        fw.consume("k", 1_000);
        assert_eq!(fw.len(), 1);
        assert_eq!(fw.cleanup_expired(1_005), 0);
        assert_eq!(fw.cleanup_expired(1_010), 1);
        assert_eq!(fw.len(), 0);
    }

    #[test]
    fn check_outcome_fields_when_blocked() {
        let fw = FixedWindow::new(1, 60);
        let _ = fw.consume("k", 1_000);
        let out = fw.consume("k", 1_010);
        assert!(!out.allowed);
        assert_eq!(out.remaining, 0);
        assert_eq!(out.retry_after_seconds, 50);
        assert_eq!(out.reset_after_seconds, 50);
    }
}
