"""Tests for ``max_keys`` and ``cleanup_interval`` (bounded memory)."""

import time

import pytest

from rust_py_rate_limit import RateLimiter


def test_defaults_are_unbounded():
    limiter = RateLimiter(limit=1, window_seconds=60)
    assert limiter.max_keys is None
    assert limiter.cleanup_interval is None
    for i in range(500):
        assert limiter.allow(f"k{i}")
    assert limiter.stats()["active_keys"] == 500


def test_full_limiter_rejects_new_keys():
    limiter = RateLimiter(limit=10, window_seconds=60, max_keys=3)
    for key in ("a", "b", "c"):
        assert limiter.allow(key)

    result = limiter.check("d")
    assert result["allowed"] is False
    assert result["retry_after_seconds"] == 60

    stats = limiter.stats()
    assert stats["active_keys"] == 3
    assert stats["rejected_full"] == 1
    assert stats["blocked"] == 1


def test_full_limiter_keeps_serving_tracked_keys():
    limiter = RateLimiter(limit=10, window_seconds=60, max_keys=2)
    limiter.allow("a")
    limiter.allow("b")
    for _ in range(100):
        limiter.allow(f"attacker-{_}")
    # The flood never displaced "a": its counter is intact and it is served.
    assert limiter.remaining("a") == 9
    assert limiter.allow("a")


def test_key_flood_stays_bounded():
    limiter = RateLimiter(limit=5, window_seconds=60, max_keys=100)
    for i in range(10_000):
        limiter.allow(f"ip-{i}")
    stats = limiter.stats()
    assert stats["active_keys"] == 100
    assert stats["rejected_full"] == 10_000 - 100


def test_sliding_algorithm_respects_max_keys():
    limiter = RateLimiter(limit=5, window_seconds=60, algorithm="sliding", max_keys=2)
    assert limiter.allow("a")
    assert limiter.allow("b")
    assert not limiter.allow("c")


def test_background_cleanup_reclaims_expired_keys():
    limiter = RateLimiter(limit=1, window_seconds=1, cleanup_interval=0.2)
    for i in range(50):
        limiter.allow(f"k{i}")
    assert limiter.stats()["active_keys"] == 50

    deadline = time.monotonic() + 5
    while limiter.stats()["active_keys"] and time.monotonic() < deadline:
        time.sleep(0.1)
    assert limiter.stats()["active_keys"] == 0


def test_dropping_limiter_stops_cleanup_thread():
    # Constructing and discarding many limiters must not leak threads or hang.
    for _ in range(20):
        RateLimiter(limit=1, window_seconds=1, cleanup_interval=0.05)


@pytest.mark.parametrize("value", [0])
def test_invalid_max_keys(value):
    with pytest.raises(ValueError):
        RateLimiter(limit=1, window_seconds=1, max_keys=value)


@pytest.mark.parametrize("value", [0, -1.0, float("inf"), float("nan")])
def test_invalid_cleanup_interval(value):
    with pytest.raises(ValueError):
        RateLimiter(limit=1, window_seconds=1, cleanup_interval=value)


def test_repr_mentions_bounds():
    limiter = RateLimiter(limit=1, window_seconds=1, max_keys=10, cleanup_interval=30)
    assert "max_keys=10" in repr(limiter)
    assert "cleanup_interval=30" in repr(limiter)
