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

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::errors::RateLimitError;
use crate::fixed_window::{CheckOutcome, FixedWindow};
use crate::sliding_window::SlidingWindow;
use crate::stats::RateLimitStats;
use crate::time_utils::now_seconds;

/// The selected algorithm backend. Both variants expose the same surface, so
/// every `RateLimiter` method just forwards to the active one.
enum Algo {
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

    fn cleanup_expired(&self, now: u64) -> usize {
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
    inner: Algo,
    stats: RateLimitStats,
}

#[pymethods]
impl RateLimiter {
    /// `RateLimiter(limit, window_seconds, algorithm="fixed")`.
    ///
    /// `limit` and `window_seconds` must be positive; `0` (or a negative value,
    /// which fails the `u64` conversion with `OverflowError`) raises
    /// `ValueError`. `algorithm` selects the strategy: `"fixed"` (default) or
    /// `"sliding"` (sliding window counter, which smooths bursts at the window
    /// boundary). Any other value raises `ValueError`.
    #[new]
    #[pyo3(signature = (limit, window_seconds, algorithm = "fixed"))]
    fn new(limit: u64, window_seconds: u64, algorithm: &str) -> PyResult<Self> {
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
        Ok(Self {
            inner,
            stats: RateLimitStats::default(),
        })
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
        let outcome = py.allow_threads(|| self.inner.consume(&key, now));
        if outcome.allowed {
            self.stats.record_allowed();
        } else {
            self.stats.record_blocked();
        }
        Ok(outcome.allowed)
    }

    /// Consumes one request for `key` and returns a dict with full detail:
    /// `allowed`, `limit`, `remaining`, `reset_after_seconds`,
    /// `retry_after_seconds`.
    fn check<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Bound<'py, PyDict>> {
        let now = now_seconds()?;
        let key_owned = key.to_string();
        let outcome = py.allow_threads(|| self.inner.consume(&key_owned, now));
        if outcome.allowed {
            self.stats.record_allowed();
        } else {
            self.stats.record_blocked();
        }

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
    /// `allowed`, `blocked`, `total_checks`, `active_keys`.
    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let snap = self.stats.snapshot(self.inner.len() as u64);
        let dict = PyDict::new(py);
        dict.set_item("allowed", snap.allowed)?;
        dict.set_item("blocked", snap.blocked)?;
        dict.set_item("total_checks", snap.total_checks)?;
        dict.set_item("active_keys", snap.active_keys)?;
        Ok(dict)
    }

    /// Returns the stats as a JSON string (handy for logging / future
    /// ImmutableLog integration).
    fn stats_json(&self) -> PyResult<String> {
        let snap = self.stats.snapshot(self.inner.len() as u64);
        serde_json::to_string(&snap).map_err(|e| RateLimitError::SystemTime(e.to_string()).into())
    }

    fn __repr__(&self) -> String {
        format!(
            "RateLimiter(limit={}, window_seconds={}, algorithm={:?})",
            self.inner.limit(),
            self.inner.window_seconds(),
            self.inner.name()
        )
    }
}
