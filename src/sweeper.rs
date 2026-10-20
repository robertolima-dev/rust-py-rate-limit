//! Background cleanup: a thread that drops expired keys on a fixed interval, so
//! memory is reclaimed without the caller having to schedule
//! `cleanup_expired()` themselves.
//!
//! Same design as `rust_py_cache/src/sweeper.rs`:
//!
//! * the thread holds a [`Weak`] reference, not an `Arc`, so it never keeps the
//!   limiter alive — once the Python object is collected, `upgrade()` fails and
//!   the thread exits on its own;
//! * the `RateLimiter` owns the [`Sweeper`], and dropping it signals the stop
//!   flag and joins the thread — no orphaned threads;
//! * the sleep is sliced into short ticks so shutdown answers the stop signal
//!   promptly instead of waiting out a whole interval.
//!
//! The thread never touches Python objects, so it runs without the GIL.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::rate_limiter::Algo;
use crate::time_utils::now_seconds;

/// Handle to the cleanup thread. Stops and joins automatically on `Drop`.
pub struct Sweeper {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Sweeper {
    /// Starts a thread that runs `cleanup_expired` on `algo` every `interval`.
    pub fn start(algo: &Arc<Algo>, interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let weak = Arc::downgrade(algo);
        let handle = thread::spawn(move || run(weak, interval, stop_thread));
        Sweeper {
            stop,
            handle: Some(handle),
        }
    }
}

/// Thread loop: sleep `interval` (in slices), then sweep, until told to stop or
/// the limiter no longer exists.
fn run(algo: Weak<Algo>, interval: Duration, stop: Arc<AtomicBool>) {
    let tick = Duration::from_millis(200);
    loop {
        let mut waited = Duration::ZERO;
        while waited < interval {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let chunk = tick.min(interval - waited);
            thread::sleep(chunk);
            waited += chunk;
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match algo.upgrade() {
            Some(a) => {
                // A clock before the epoch only skips this round; the next
                // tick tries again.
                if let Ok(now) = now_seconds() {
                    a.cleanup_expired(now);
                }
            }
            None => return, // the limiter was collected; nothing left to sweep
        }
    }
}

impl Drop for Sweeper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
