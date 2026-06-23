"""Tests for ``stats()`` and concurrency."""

import threading

from rust_py_rate_limit import RateLimiter


def test_stats_counts_allowed():
    limiter = RateLimiter(limit=10, window_seconds=60)
    for _ in range(5):
        limiter.allow("k")
    stats = limiter.stats()
    assert stats["allowed"] == 5
    assert stats["blocked"] == 0
    assert stats["total_checks"] == 5


def test_stats_counts_blocked():
    limiter = RateLimiter(limit=2, window_seconds=60)
    for _ in range(5):
        limiter.allow("k")
    stats = limiter.stats()
    assert stats["allowed"] == 2
    assert stats["blocked"] == 3
    assert stats["total_checks"] == 5


def test_stats_active_keys():
    limiter = RateLimiter(limit=10, window_seconds=60)
    limiter.allow("a")
    limiter.allow("b")
    limiter.allow("c")
    assert limiter.stats()["active_keys"] == 3


def test_thread_safety_counts_are_consistent():
    # With limit < total requests, exactly `limit` should be admitted within a
    # single window regardless of how many threads race on the same key.
    limit = 500
    threads_count = 8
    per_thread = 250  # 8 * 250 = 2000 total requests
    limiter = RateLimiter(limit=limit, window_seconds=60)

    def worker():
        for _ in range(per_thread):
            limiter.allow("shared")

    threads = [threading.Thread(target=worker) for _ in range(threads_count)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    stats = limiter.stats()
    total = threads_count * per_thread
    assert stats["total_checks"] == total
    assert stats["allowed"] == limit
    assert stats["blocked"] == total - limit


def test_stats_json_roundtrip():
    import json

    limiter = RateLimiter(limit=3, window_seconds=60)
    limiter.allow("k")
    parsed = json.loads(limiter.stats_json())
    assert parsed["allowed"] == 1
    assert parsed["active_keys"] == 1
