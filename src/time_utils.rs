//! Time helpers.
//!
//! The Fixed Window algorithm only needs a monotonic-ish notion of "seconds
//! since the Unix epoch". We read it from `std::time::SystemTime` and convert
//! any failure into a `RateLimitError` instead of unwrapping.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::errors::RateLimitError;

/// Returns the current time as whole seconds since the Unix epoch.
///
/// Returns `Err` if the system clock is set before the Unix epoch, which keeps
/// the critical path free of `.unwrap()`.
pub fn now_seconds() -> Result<u64, RateLimitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| RateLimitError::SystemTime(e.to_string()))
}
