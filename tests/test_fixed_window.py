"""Core Fixed Window behavior: allow / check / window reset / invalid params."""

import time

import pytest

from rust_py_rate_limit import RateLimiter


def test_allows_up_to_limit():
    limiter = RateLimiter(limit=3, window_seconds=60)
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True


def test_blocks_after_limit():
    limiter = RateLimiter(limit=2, window_seconds=60)
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False


def test_resets_after_window():
    limiter = RateLimiter(limit=1, window_seconds=1)
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False

    time.sleep(1.1)

    assert limiter.allow("user:1") is True


def test_check_allowed_payload():
    limiter = RateLimiter(limit=100, window_seconds=60)
    result = limiter.check("user:1")
    assert result["allowed"] is True
    assert result["limit"] == 100
    assert result["remaining"] == 99
    assert result["reset_after_seconds"] <= 60
    assert result["retry_after_seconds"] == 0


def test_check_blocked_payload():
    limiter = RateLimiter(limit=1, window_seconds=60)
    limiter.check("user:1")
    result = limiter.check("user:1")
    assert result["allowed"] is False
    assert result["limit"] == 1
    assert result["remaining"] == 0
    assert result["retry_after_seconds"] > 0
    assert result["retry_after_seconds"] == result["reset_after_seconds"]


def test_independent_keys():
    limiter = RateLimiter(limit=1, window_seconds=60)
    assert limiter.allow("a") is True
    assert limiter.allow("b") is True
    assert limiter.allow("a") is False
    assert limiter.allow("c") is True


def test_properties():
    limiter = RateLimiter(limit=7, window_seconds=30)
    assert limiter.max_requests == 7
    assert limiter.window_seconds == 30


@pytest.mark.parametrize("limit", [0])
def test_zero_limit_raises(limit):
    # Defined behavior: a non-positive limit is invalid and raises ValueError.
    with pytest.raises(ValueError):
        RateLimiter(limit=limit, window_seconds=60)


def test_zero_window_raises():
    with pytest.raises(ValueError):
        RateLimiter(limit=10, window_seconds=0)


def test_negative_params_raise():
    with pytest.raises((ValueError, OverflowError)):
        RateLimiter(limit=-1, window_seconds=60)
