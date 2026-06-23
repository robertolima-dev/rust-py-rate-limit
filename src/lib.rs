//! rust-py-rate-limit — Fast local rate limiting for Python, powered by Rust.
//!
//! The compiled extension is exposed to Python as `rust_py_rate_limit._core`.
//! The pure-Python package (`python/rust_py_rate_limit`) re-exports
//! [`RateLimiter`] and layers framework integrations on top.

mod entry;
mod errors;
mod fixed_window;
mod rate_limiter;
mod stats;
mod time_utils;

// Reserved for future algorithms (see the roadmap). They are intentionally not
// wired into the module yet so the MVP stays focused on Fixed Window.
// mod sliding_window;
// mod token_bucket;

use pyo3::prelude::*;

use rate_limiter::RateLimiter;

/// Simple smoke-test function exposed during bootstrapping.
#[pyfunction]
fn hello() -> &'static str {
    "Hello from rust-py-rate-limit! Fast local rate limiting for Python, powered by Rust."
}

/// The native extension module: `rust_py_rate_limit._core`.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(hello, m)?)?;
    m.add_class::<RateLimiter>()?;
    Ok(())
}
