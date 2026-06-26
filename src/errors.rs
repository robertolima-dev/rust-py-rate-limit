//! Error types for the rate limiter.
//!
//! We use `thiserror` to derive ergonomic error messages and implement
//! `From<RateLimitError> for PyErr` so that errors raised in the Rust core are
//! surfaced to Python as proper exceptions (no `panic!`/`.unwrap()` on the
//! critical path).

use pyo3::exceptions::PyValueError;
use pyo3::PyErr;
use thiserror::Error;

/// Errors that can occur while configuring or operating the rate limiter.
#[derive(Error, Debug)]
pub enum RateLimitError {
    /// `limit` must be a positive integer.
    #[error("invalid `limit`: must be greater than 0")]
    InvalidLimit,

    /// `window_seconds` must be a positive integer.
    #[error("invalid `window_seconds`: must be greater than 0")]
    InvalidWindow,

    /// `algorithm` must be one of the supported strategies.
    #[error("invalid `algorithm`: {0:?} (expected \"fixed\" or \"sliding\")")]
    InvalidAlgorithm(String),

    /// The system clock could not be read.
    #[error("system time error: {0}")]
    SystemTime(String),
}

impl From<RateLimitError> for PyErr {
    fn from(err: RateLimitError) -> PyErr {
        // Every configuration/runtime error maps to a Python `ValueError`,
        // which is the most idiomatic exception for invalid arguments.
        PyValueError::new_err(err.to_string())
    }
}
