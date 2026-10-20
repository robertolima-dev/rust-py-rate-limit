//! The Python-facing `RateLimiter` class.
//!
//! This is a thin `#[pyclass]` wrapper around one of the algorithm backends
//! ([`FixedWindow`] or [`SlidingWindow`]) plus the activity counters. The heavy
//! lifting (the per-key map operations) runs inside `py.allow_threads(...)`,
//! which releases the GIL so multiple Python threads can hit the limiter
//! concurrently and exercise DashMap's per-shard locking.
//!
//! The class is marked `subclass` so the pure-Python layer can subclass it to
//! add the `.limit(...)` decorator while keeping these methods native.
//!
//! ## Bounded memory
//!
//! The usual key is a client IP or token — something the caller controls. With
//! no bound, a client minting fresh keys grows the map until the process dies.
//! `max_keys` caps it: when the map is full, expired keys are reclaimed first
//! (at most once per second, so a flood can't make every request pay an O(n)
//! sweep) and, if it is still full, the *new* key is rejected. Tracked keys are
//! never evicted to make room — evicting would reset a legitimate client's
//! counter on the attacker's behalf. `cleanup_interval` adds a background
//! thread that reclaims expired keys on a timer (see [`crate::sweeper`]).
//!
//! The cap is checked before the insert without a global lock, so under heavy
//! concurrency the map can overshoot `max_keys` by at most the number of
//! threads racing on new keys at that instant.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::errors::RateLimitError;
use crate::fixed_window::{CheckOutcome, FixedWindow};
use crate::sliding_window::SlidingWindow;
use crate::stats::RateLimitStats;
use crate::sweeper::Sweeper;
use crate::time_utils::now_seconds;

/// The selected algorithm backend. Both variants expose the same surface, so
/// every `RateLimiter` method just forwards to the active one.
pub(crate) enum Algo {
    Fixed(FixedWindow),
    Sliding(SlidingWindow),
}

impl Algo {
    fn limit(&self) -> u64 {
        match self {
            Algo::Fixed(a) => a.limit,
            Algo::Sliding(a) => a.limit,
        }
    }

    fn window_seconds(&self) -> u64 {
        match self {
            Algo::Fixed(a) => a.window_seconds,
            Algo::Sliding(a) => a.window_seconds,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Algo::Fixed(_) => "fixed",
            Algo::Sliding(_) => "sliding",
        }
    }

    fn consume(&self, key: &str, now: u64) -> CheckOutcome {
        match self {
            Algo::Fixed(a) => a.consume(key, now),
            Algo::Sliding(a) => a.consume(key, now),
        }
    }

    fn remaining(&self, key: &str, now: u64) -> u64 {
        match self {
            Algo::Fixed(a) => a.remaining(key, now),
            Algo::Sliding(a) => a.remaining(key, now),
        }
    }

    fn reset(&self, key: &str) -> bool {
        match self {
            Algo::Fixed(a) => a.reset(key),
            Algo::Sliding(a) => a.reset(key),
        }
    }

    fn clear(&self) {
        match self {
            Algo::Fixed(a) => a.clear(),
            Algo::Sliding(a) => a.clear(),
        }
    }

    fn contains(&self, key: &str) -> bool {
        match self {
            Algo::Fixed(a) => a.contains(key),
            Algo::Sliding(a) => a.contains(key),
        }
    }

    pub(crate) fn cleanup_expired(&self, now: u64) -> usize {
        match self {
            Algo::Fixed(a) => a.cleanup_expired(now),
            Algo::Sliding(a) => a.cleanup_expired(now),
        }
    }

    fn len(&self) -> usize {
        match self {
            Algo::Fixed(a) => a.len(),
            Algo::Sliding(a) => a.len(),
        }
    }
}

#[pyclass(subclass, name = "RateLimiter", module = "rust_py_rate_limit._core")]
pub struct RateLimiter {
    inner: Arc<Algo>,
    stats: RateLimitStats,
    max_keys: Option<usize>,
    cleanup_interval: Option<f64>,
    /// Second (epoch) of the last inline sweep triggered by a full map.
    last_full_sweep: AtomicU64,
    /// Background cleanup thread; stopped and joined when the limiter drops.
    _sweeper: Option<Sweeper>,
}

/// Result of [`RateLimiter::evaluate`]: the outcome, plus whether it was a
/// rejection caused by the `max_keys` cap rather than by the key's own limit.
struct Evaluation {
    outcome: CheckOutcome,
    rejected_full: bool,
}

impl RateLimiter {
    /// Runs one request through the capacity check and the algorithm. Called
    /// with the GIL released.
    fn evaluate(&self, key: &str, now: u64) -> Evaluation {
        if let Some(max) = self.max_keys {
            if self.inner.len() >= max && !self.inner.contains(key) {
                // Reclaim expired keys, but only once per second: under a
                // flood of new keys this branch runs on every request.
                let last = self.last_full_sweep.load(Ordering::Relaxed);
                if now > last
                    && self
                        .last_full_sweep
                        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                {
                    self.inner.cleanup_expired(now);
                }
                if self.inner.len() >= max {
                    let window = self.inner.window_seconds();
                    return Evaluation {
                        outcome: CheckOutcome {
                            allowed: false,
                            limit: self.inner.limit(),
                            remaining: 0,
                            reset_after_seconds: window,
                            retry_after_seconds: window,
                        },
                        rejected_full: true,
                    };
                }
            }
        }
        Evaluation {
            outcome: self.inner.consume(key, now),
            rejected_full: false,
        }
    }

    fn record(&self, eval: &Evaluation) {
        if eval.rejected_full {
            self.stats.record_rejected_full();
        } else if eval.outcome.allowed {
            self.stats.record_allowed();
        } else {
            self.stats.record_blocked();
        }
    }
}

#[pymethods]
impl RateLimiter {
    /// `RateLimiter(limit, window_seconds, algorithm="fixed", max_keys=None,
    /// cleanup_interval=None)`.
    ///
    /// `limit` and `window_seconds` must be positive; `0` (or a negative value,
    /// which fails the `u64` conversion with `OverflowError`) raises
    /// `ValueError`. `algorithm` selects the strategy: `"fixed"` (default) or
    /// `"sliding"` (sliding window counter, which smooths bursts at the window
    /// boundary). Any other value raises `ValueError`.
    ///
    /// `max_keys` caps how many keys are tracked at once (see the module docs);
    /// `None` (default) leaves it unbounded. `cleanup_interval` (seconds)
    /// starts a background thread that drops expired keys on that period;
    /// `None` (default) leaves cleanup to `cleanup_expired()` and to the
    /// inline sweep a full map triggers.
    #[new]
    #[pyo3(signature = (limit, window_seconds, algorithm = "fixed", max_keys = None, cleanup_interval = None))]
    fn new(
        limit: u64,
        window_seconds: u64,
        algorithm: &str,
        max_keys: Option<usize>,
        cleanup_interval: Option<f64>,
    ) -> PyResult<Self> {
        if limit == 0 {
            return Err(RateLimitError::InvalidLimit.into());
        }
        if window_seconds == 0 {
            return Err(RateLimitError::InvalidWindow.into());
        }
        let inner = match algorithm {
            "fixed" => Algo::Fixed(FixedWindow::new(limit, window_seconds)),
            "sliding" => Algo::Sliding(SlidingWindow::new(limit, window_seconds)),
            other => return Err(RateLimitError::InvalidAlgorithm(other.to_string()).into()),
        };
        if max_keys == Some(0) {
            return Err(RateLimitError::InvalidMaxKeys.into());
        }
        let interval = match cleanup_interval {
            None => None,
            Some(secs) if secs.is_finite() && secs > 0.0 => Some(Duration::from_secs_f64(secs)),
            Some(_) => return Err(RateLimitError::InvalidCleanupInterval.into()),
        };
        let inner = Arc::new(inner);
        let sweeper = interval.map(|i| Sweeper::start(&inner, i));
        Ok(Self {
            inner,
            stats: RateLimitStats::default(),
            max_keys,
            cleanup_interval,
            last_full_sweep: AtomicU64::new(0),
            _sweeper: sweeper,
        })
    }

    /// The configured key cap, or `None` when unbounded.
    #[getter]
    fn max_keys(&self) -> Option<usize> {
        self.max_keys
    }

    /// The background cleanup period in seconds, or `None` when disabled.
    #[getter]
    fn cleanup_interval(&self) -> Option<f64> {
        self.cleanup_interval
    }

    /// The configured maximum number of requests per window.
    ///
    /// Exposed as `max_requests` rather than `limit` because the pure-Python
    /// subclass uses `.limit(...)` as the decorator factory.
    #[getter]
    fn max_requests(&self) -> u64 {
        self.inner.limit()
    }

    /// The configured window length in seconds.
    #[getter]
    fn window_seconds(&self) -> u64 {
        self.inner.window_seconds()
    }

    /// The active algorithm: `"fixed"` or `"sliding"`.
    #[getter]
    fn algorithm(&self) -> &'static str {
        self.inner.name()
    }

    /// Consumes one request for `key`. Returns `True` if admitted.
    fn allow(&self, py: Python<'_>, key: &str) -> PyResult<bool> {
        let now = now_seconds()?;
        let key = key.to_string();
        let eval = py.allow_threads(|| self.evaluate(&key, now));
        self.record(&eval);
        Ok(eval.outcome.allowed)
    }

    /// Consumes one request for `key` and returns a dict with full detail:
    /// `allowed`, `limit`, `remaining`, `reset_after_seconds`,
    /// `retry_after_seconds`.
    fn check<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Bound<'py, PyDict>> {
        let now = now_seconds()?;
        let key_owned = key.to_string();
        let eval = py.allow_threads(|| self.evaluate(&key_owned, now));
        self.record(&eval);
        let outcome = eval.outcome;

        let dict = PyDict::new(py);
        dict.set_item("allowed", outcome.allowed)?;
        dict.set_item("limit", outcome.limit)?;
        dict.set_item("remaining", outcome.remaining)?;
        dict.set_item("reset_after_seconds", outcome.reset_after_seconds)?;
        dict.set_item("retry_after_seconds", outcome.retry_after_seconds)?;
        Ok(dict)
    }

    /// Returns how many requests remain for `key` without consuming one.
    fn remaining(&self, py: Python<'_>, key: &str) -> PyResult<u64> {
        let now = now_seconds()?;
        let key = key.to_string();
        Ok(py.allow_threads(|| self.inner.remaining(&key, now)))
    }

    /// Removes `key`'s state. Returns `True` if the key existed.
    fn reset(&self, key: &str) -> bool {
        self.inner.reset(key)
    }

    /// Removes all keys.
    fn clear(&self) {
        self.inner.clear();
    }

    /// Removes expired keys lazily. Returns the number removed.
    fn cleanup_expired(&self, py: Python<'_>) -> PyResult<usize> {
        let now = now_seconds()?;
        Ok(py.allow_threads(|| self.inner.cleanup_expired(now)))
    }

    /// Returns a dict of activity counters:
    /// `allowed`, `blocked`, `total_checks`, `active_keys`, `rejected_full`
    /// (new keys turned away by `max_keys`; also counted in `blocked`).
    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let snap = self.stats.snapshot(self.inner.len() as u64);
        let dict = PyDict::new(py);
        dict.set_item("allowed", snap.allowed)?;
        dict.set_item("blocked", snap.blocked)?;
        dict.set_item("total_checks", snap.total_checks)?;
        dict.set_item("active_keys", snap.active_keys)?;
        dict.set_item("rejected_full", snap.rejected_full)?;
        Ok(dict)
    }

    /// Returns the stats as a JSON string (handy for logging / future
    /// ImmutableLog integration).
    fn stats_json(&self) -> PyResult<String> {
        let snap = self.stats.snapshot(self.inner.len() as u64);
        serde_json::to_string(&snap).map_err(|e| RateLimitError::SystemTime(e.to_string()).into())
    }

    fn __repr__(&self) -> String {
        let mut repr = format!(
            "RateLimiter(limit={}, window_seconds={}, algorithm={:?}",
            self.inner.limit(),
            self.inner.window_seconds(),
            self.inner.name()
        );
        if let Some(max) = self.max_keys {
            repr.push_str(&format!(", max_keys={max}"));
        }
        if let Some(secs) = self.cleanup_interval {
            repr.push_str(&format!(", cleanup_interval={secs}"));
        }
        repr.push(')');
        repr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(limit: u64, window: u64, max_keys: Option<usize>) -> RateLimiter {
        RateLimiter {
            inner: Arc::new(Algo::Fixed(FixedWindow::new(limit, window))),
            stats: RateLimitStats::default(),
            max_keys,
            cleanup_interval: None,
            last_full_sweep: AtomicU64::new(0),
            _sweeper: None,
        }
    }

    #[test]
    fn full_map_rejects_new_keys_but_not_tracked_ones() {
        let rl = limiter(5, 60, Some(2));
        assert!(rl.evaluate("a", 1_000).outcome.allowed);
        assert!(rl.evaluate("b", 1_000).outcome.allowed);
        let c = rl.evaluate("c", 1_000);
        assert!(!c.outcome.allowed);
        assert!(c.rejected_full);
        assert_eq!(c.outcome.retry_after_seconds, 60);
        // Keys already tracked keep being served normally.
        assert!(rl.evaluate("a", 1_000).outcome.allowed);
        assert_eq!(rl.inner.len(), 2);
    }

    #[test]
    fn full_map_reclaims_expired_keys_before_rejecting() {
        let rl = limiter(5, 10, Some(2));
        rl.evaluate("a", 1_000);
        rl.evaluate("b", 1_000);
        // Both windows are over by t=1_010, so "c" fits after the inline sweep.
        let c = rl.evaluate("c", 1_010);
        assert!(c.outcome.allowed);
        assert!(!c.rejected_full);
        assert_eq!(rl.inner.len(), 1);
    }

    #[test]
    fn inline_sweep_runs_at_most_once_per_second() {
        let rl = limiter(5, 10, Some(1));
        rl.evaluate("a", 1_000);
        // Same second as the first sweep attempt: no second sweep.
        assert!(rl.evaluate("b", 1_005).rejected_full);
        assert_eq!(rl.last_full_sweep.load(Ordering::Relaxed), 1_005);
        // "a" expires at 1_010, but a sweep already ran in second 1_005 —
        // the next one is allowed from 1_006 on.
        assert!(!rl.evaluate("b", 1_010).rejected_full);
    }

    #[test]
    fn unbounded_by_default() {
        let rl = limiter(1, 60, None);
        for i in 0..1_000 {
            assert!(rl.evaluate(&format!("k{i}"), 1_000).outcome.allowed);
        }
        assert_eq!(rl.inner.len(), 1_000);
    }
}
