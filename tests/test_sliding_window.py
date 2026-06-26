"""Sliding Window algorithm: selection, validation, and admit/block surface.

The boundary-smoothing math (weighting the previous window) is verified
deterministically in the Rust unit tests (`src/sliding_window.rs`), where the
clock can be injected. Here we cover the Python-facing API: algorithm
selection, the new ``algorithm`` getter, validation, and basic behavior.
"""

import time

import pytest

from rust_py_rate_limit import RateLimiter


def test_algorithm_defaults_to_fixed():
    limiter = RateLimiter(limit=5, window_seconds=60)
    assert limiter.algorithm == "fixed"


def test_algorithm_getter_reports_sliding():
    limiter = RateLimiter(limit=5, window_seconds=60, algorithm="sliding")
    assert limiter.algorithm == "sliding"


def test_invalid_algorithm_raises():
    with pytest.raises(ValueError):
        RateLimiter(limit=5, window_seconds=60, algorithm="leaky")


def test_sliding_admits_up_to_limit():
    limiter = RateLimiter(limit=3, window_seconds=60, algorithm="sliding")
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True


def test_sliding_blocks_after_limit():
    limiter = RateLimiter(limit=2, window_seconds=60, algorithm="sliding")
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False


def test_sliding_remaining_reflects_consumption():
    limiter = RateLimiter(limit=3, window_seconds=60, algorithm="sliding")
    assert limiter.remaining("user:1") == 3
    limiter.allow("user:1")
    assert limiter.remaining("user:1") == 2


def test_sliding_check_payload_when_blocked():
    limiter = RateLimiter(limit=1, window_seconds=60, algorithm="sliding")
    assert limiter.check("user:1")["allowed"] is True
    blocked = limiter.check("user:1")
    assert blocked["allowed"] is False
    assert blocked["remaining"] == 0
    assert blocked["retry_after_seconds"] > 0


def test_sliding_keys_are_independent():
    limiter = RateLimiter(limit=1, window_seconds=60, algorithm="sliding")
    assert limiter.allow("a") is True
    assert limiter.allow("b") is True
    assert limiter.allow("a") is False


def test_sliding_stats_and_repr():
    limiter = RateLimiter(limit=1, window_seconds=60, algorithm="sliding")
    limiter.allow("user:1")
    limiter.allow("user:1")  # blocked
    stats = limiter.stats()
    assert stats["allowed"] == 1
    assert stats["blocked"] == 1
    assert stats["active_keys"] == 1
    assert 'algorithm="sliding"' in repr(limiter)


def test_sliding_recovers_after_two_windows():
    limiter = RateLimiter(limit=1, window_seconds=1, algorithm="sliding")
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False
    # After two full windows the previous-window weight has fully decayed.
    time.sleep(2.1)
    assert limiter.allow("user:1") is True
