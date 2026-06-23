//! Lock-free counters describing rate limiter activity.
//!
//! We use `AtomicU64` with `Relaxed` ordering: the counters are independent of
//! each other and of the per-key state, so we only need atomicity (no lost
//! updates under concurrency), not cross-counter ordering guarantees. This
//! keeps statistics off the critical path's lock and cheap to update.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

#[derive(Debug, Default)]
pub struct RateLimitStats {
    pub allowed: AtomicU64,
    pub blocked: AtomicU64,
    pub total_checks: AtomicU64,
}

/// A point-in-time, plain-data view of the stats (used for serialization and
/// for handing values back to Python).
#[derive(Debug, Serialize)]
pub struct StatsSnapshot {
    pub allowed: u64,
    pub blocked: u64,
    pub total_checks: u64,
    pub active_keys: u64,
}

impl RateLimitStats {
    /// Records one admitted request.
    pub fn record_allowed(&self) {
        self.allowed.fetch_add(1, Ordering::Relaxed);
        self.total_checks.fetch_add(1, Ordering::Relaxed);
    }

    /// Records one rejected request.
    pub fn record_blocked(&self) {
        self.blocked.fetch_add(1, Ordering::Relaxed);
        self.total_checks.fetch_add(1, Ordering::Relaxed);
    }

    /// Builds a snapshot. `active_keys` is supplied by the caller because it is
    /// derived from the live map length rather than an atomic counter (which
    /// would drift on cleanup).
    pub fn snapshot(&self, active_keys: u64) -> StatsSnapshot {
        StatsSnapshot {
            allowed: self.allowed.load(Ordering::Relaxed),
            blocked: self.blocked.load(Ordering::Relaxed),
            total_checks: self.total_checks.load(Ordering::Relaxed),
            active_keys,
        }
    }
}
