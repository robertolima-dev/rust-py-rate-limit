//! Sliding Window (counter) rate limiting algorithm.
//!
//! The Fixed Window algorithm has a well-known weakness: a client can send
//! `limit` requests at the very end of one window and `limit` more at the very
//! start of the next, pushing `2 * limit` requests through in a span shorter
//! than a single window. The **sliding window counter** smooths that boundary
//! by keeping the count for the *current* aligned window plus the count for the
//! *previous* one, and weighting the previous window by how much of it still
//! overlaps the trailing `window_seconds` ending at `now`:
//!
//! ```text
//! estimated = previous_count * weight + current_count
//! weight    = (window_seconds - elapsed_in_current_window) / window_seconds
//! ```
//!
//! This is an approximation (it assumes the previous window's requests were
//! spread evenly), but it is O(1) in time and memory per key — unlike a sliding
//! *log*, which stores every timestamp — and it removes the doubling at the
//! boundary. The estimate is only ever an over- or under-count by a fraction of
//! one window, which is the standard, production-friendly trade-off.
//!
//! ## Concurrency
//!
//! Like [`FixedWindow`](crate::fixed_window::FixedWindow), per-key state lives in
//! a sharded [`DashMap`], so different keys never contend on a global lock.

use dashmap::DashMap;

use crate::fixed_window::CheckOutcome;

/// Per-key bookkeeping for the sliding window counter.
///
/// `window_index` is the aligned window number (`now / window_seconds`) that
/// `current` counts. `previous` is the count for `window_index - 1`.
#[derive(Debug, Clone)]
struct SlidingEntry {
    window_index: u64,
    current: u64,
    previous: u64,
}

impl SlidingEntry {
    fn new(window_index: u64) -> Self {
        Self {
            window_index,
            current: 0,
            previous: 0,
        }
    }

    /// Rolls `current`/`previous` forward so they describe the window that
    /// contains `idx`. A one-step advance shifts `current` into `previous`; a
    /// larger jump means both windows are stale, so they reset to zero.
    fn advance_to(&mut self, idx: u64) {
        if idx == self.window_index {
            // same window — nothing to do
        } else if idx == self.window_index + 1 {
            self.previous = self.current;
            self.current = 0;
            self.window_index = idx;
        } else {
            // `idx < window_index` only happens if the clock went backwards; in
            // every case the old counts no longer overlap, so start fresh.
            self.previous = 0;
            self.current = 0;
            self.window_index = idx;
        }
    }
}

/// The weight applied to the previous window: the fraction of `window_seconds`
/// still ahead of `now` within the current window.
fn weight(window_seconds: u64, elapsed: u64) -> f64 {
    let remaining_in_window = window_seconds.saturating_sub(elapsed);
    remaining_in_window as f64 / window_seconds as f64
}

/// Sliding Window limiter state, independent of any Python types. Mirrors the
/// surface of [`FixedWindow`](crate::fixed_window::FixedWindow) so the
/// `RateLimiter` wrapper can dispatch to either without special-casing.
pub struct SlidingWindow {
    pub limit: u64,
    pub window_seconds: u64,
    entries: DashMap<String, SlidingEntry>,
}

impl SlidingWindow {
    pub fn new(limit: u64, window_seconds: u64) -> Self {
        Self {
            limit,
            window_seconds,
            entries: DashMap::new(),
        }
    }

    fn index(&self, now: u64) -> u64 {
        now / self.window_seconds
    }

    /// Evaluates `key` at `now`, admitting the request if the weighted estimate
    /// stays under `limit`. Shared by `allow` and `check`.
    pub fn consume(&self, key: &str, now: u64) -> CheckOutcome {
        let idx = self.index(now);
        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(|| SlidingEntry::new(idx));
        entry.advance_to(idx);

        let elapsed = now - idx * self.window_seconds;
        let w = weight(self.window_seconds, elapsed);
        let estimated = entry.previous as f64 * w + entry.current as f64;

        let allowed = estimated < self.limit as f64;
        if allowed {
            entry.current += 1;
        }

        let estimated_after = entry.previous as f64 * w + entry.current as f64;
        let remaining = (self.limit as f64 - estimated_after).max(0.0).floor() as u64;
        let reset_after = self.window_seconds.saturating_sub(elapsed);

        CheckOutcome {
            allowed,
            limit: self.limit,
            remaining,
            reset_after_seconds: reset_after,
            retry_after_seconds: if allowed { 0 } else { reset_after },
        }
    }

    /// Requests remaining for `key` at `now` without consuming one. An unknown
    /// key has the full limit available.
    pub fn remaining(&self, key: &str, now: u64) -> u64 {
        let idx = self.index(now);
        match self.entries.get(key) {
            Some(entry) => {
                // Compute the effective (rolled-forward) counts without mutating.
                let (previous, current) = if idx == entry.window_index {
                    (entry.previous, entry.current)
                } else if idx == entry.window_index + 1 {
                    (entry.current, 0)
                } else {
                    (0, 0)
                };
                let elapsed = now - idx * self.window_seconds;
                let w = weight(self.window_seconds, elapsed);
                let estimated = previous as f64 * w + current as f64;
                (self.limit as f64 - estimated).max(0.0).floor() as u64
            }
            None => self.limit,
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

    /// Removes every key whose windows can no longer contribute (the key has
    /// not been seen for at least two full windows). Returns the number removed.
    pub fn cleanup_expired(&self, now: u64) -> usize {
        let idx = self.index(now);
        let mut removed = 0usize;
        self.entries.retain(|_key, entry| {
            // Stale once `now` is two or more windows past the tracked one:
            // neither `current` nor `previous` overlaps the trailing window.
            let stale = idx >= entry.window_index.saturating_add(2);
            if stale {
                removed += 1;
            }
            !stale
        });
        removed
    }

    /// Number of keys currently tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_up_to_limit_within_one_window() {
        let sw = SlidingWindow::new(3, 60);
        // All at t=0 (start of window): previous=0, weight irrelevant.
        assert!(sw.consume("k", 0).allowed);
        assert!(sw.consume("k", 0).allowed);
        assert!(sw.consume("k", 0).allowed);
        assert!(!sw.consume("k", 0).allowed);
    }

    #[test]
    fn smooths_the_boundary_burst() {
        // limit=10/60s. Fill the first window, then cross into the next at its
        // midpoint: the previous window still counts at weight 0.5, so only a
        // few requests get through instead of a full fresh 10.
        let sw = SlidingWindow::new(10, 60);
        for _ in 0..10 {
            assert!(sw.consume("k", 30).allowed); // window 0 (idx 0), elapsed 30
        }
        // t=90 → idx 1, elapsed 30, weight = (60-30)/60 = 0.5.
        // estimated = previous(10)*0.5 + current(0) = 5.0 → 5 slots free.
        assert_eq!(sw.remaining("k", 90), 5);
        let mut admitted = 0;
        for _ in 0..10 {
            if sw.consume("k", 90).allowed {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 5, "boundary burst must be throttled to 5, not 10");
    }

    #[test]
    fn fully_resets_after_two_windows() {
        let sw = SlidingWindow::new(2, 10);
        assert!(sw.consume("k", 0).allowed);
        assert!(sw.consume("k", 0).allowed);
        assert!(!sw.consume("k", 0).allowed);
        // Two windows later (t=20 → idx 2, previous idx 0 is stale): full limit.
        assert_eq!(sw.remaining("k", 20), 2);
        assert!(sw.consume("k", 20).allowed);
    }

    #[test]
    fn keys_are_independent() {
        let sw = SlidingWindow::new(1, 60);
        assert!(sw.consume("a", 0).allowed);
        assert!(sw.consume("b", 0).allowed);
        assert!(!sw.consume("a", 0).allowed);
    }

    #[test]
    fn cleanup_removes_only_stale_keys() {
        let sw = SlidingWindow::new(1, 10);
        sw.consume("k", 0); // idx 0
        assert_eq!(sw.len(), 1);
        // t=10 → idx 1: previous window still overlaps, keep it.
        assert_eq!(sw.cleanup_expired(10), 0);
        // t=20 → idx 2: two windows on, stale.
        assert_eq!(sw.cleanup_expired(20), 1);
        assert_eq!(sw.len(), 0);
    }

    #[test]
    fn blocked_outcome_reports_retry_after() {
        let sw = SlidingWindow::new(1, 60);
        let _ = sw.consume("k", 0);
        let out = sw.consume("k", 10); // same window, blocked
        assert!(!out.allowed);
        assert_eq!(out.remaining, 0);
        assert_eq!(out.reset_after_seconds, 50);
        assert_eq!(out.retry_after_seconds, 50);
    }
}
