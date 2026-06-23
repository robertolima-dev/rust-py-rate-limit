"""Tests for ``remaining()``."""

import time

from rust_py_rate_limit import RateLimiter


def test_remaining_full_for_unknown_key():
    limiter = RateLimiter(limit=5, window_seconds=60)
    assert limiter.remaining("never-seen") == 5


def test_remaining_decreases_with_calls():
    limiter = RateLimiter(limit=5, window_seconds=60)
    assert limiter.remaining("k") == 5
    limiter.allow("k")
    assert limiter.remaining("k") == 4
    limiter.allow("k")
    assert limiter.remaining("k") == 3


def test_remaining_does_not_consume():
    limiter = RateLimiter(limit=2, window_seconds=60)
    limiter.allow("k")
    # Peeking many times must not change the count.
    for _ in range(10):
        assert limiter.remaining("k") == 1
    assert limiter.allow("k") is True
    assert limiter.allow("k") is False


def test_remaining_zero_when_exhausted():
    limiter = RateLimiter(limit=1, window_seconds=60)
    limiter.allow("k")
    assert limiter.remaining("k") == 0


def test_remaining_resets_after_window():
    limiter = RateLimiter(limit=1, window_seconds=1)
    limiter.allow("k")
    assert limiter.remaining("k") == 0
    time.sleep(1.1)
    assert limiter.remaining("k") == 1
