//! Per-key bookkeeping for the Fixed Window algorithm.

use serde::{Deserialize, Serialize};

/// A single key's counter within the current fixed window.
///
/// * `count` — how many requests have been admitted in the current window.
/// * `window_start` — Unix timestamp (seconds) when the current window began.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEntry {
    pub count: u64,
    pub window_start: u64,
}

impl RateLimitEntry {
    /// Creates a fresh entry whose window starts at `window_start`.
    pub fn new(window_start: u64) -> Self {
        Self {
            count: 0,
            window_start,
        }
    }

    /// Returns `true` if the window that began at `window_start` has elapsed.
    pub fn is_expired(&self, now: u64, window_seconds: u64) -> bool {
        now >= self.window_start.saturating_add(window_seconds)
    }
}
