"""Tests for ``reset()``, ``clear()`` and ``cleanup_expired()``."""

import time

from rust_py_rate_limit import RateLimiter


def test_reset_removes_key():
    limiter = RateLimiter(limit=1, window_seconds=60)
    limiter.allow("k")
    assert limiter.allow("k") is False

    assert limiter.reset("k") is True
    # After reset the key starts fresh.
    assert limiter.allow("k") is True


def test_reset_unknown_key_returns_false():
    limiter = RateLimiter(limit=1, window_seconds=60)
    assert limiter.reset("nope") is False


def test_clear_removes_everything():
    limiter = RateLimiter(limit=1, window_seconds=60)
    limiter.allow("a")
    limiter.allow("b")
    assert limiter.allow("a") is False
    assert limiter.allow("b") is False

    limiter.clear()

    assert limiter.allow("a") is True
    assert limiter.allow("b") is True


def test_cleanup_expired_removes_expired_keys():
    limiter = RateLimiter(limit=1, window_seconds=1)
    limiter.allow("a")
    limiter.allow("b")
    assert limiter.stats()["active_keys"] == 2

    # Not expired yet.
    assert limiter.cleanup_expired() == 0

    time.sleep(1.1)
    removed = limiter.cleanup_expired()
    assert removed == 2
    assert limiter.stats()["active_keys"] == 0
