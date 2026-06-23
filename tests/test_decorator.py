"""Tests for the ``@limiter.limit(...)`` decorator and ``rate_limit`` helper."""

import pytest

from rust_py_rate_limit import RateLimiter, RateLimitExceeded, rate_limit


def test_decorator_allows_then_raises():
    limiter = RateLimiter(limit=2, window_seconds=60)

    @limiter.limit("login")
    def login():
        return "ok"

    assert login() == "ok"
    assert login() == "ok"
    with pytest.raises(RateLimitExceeded) as exc:
        login()
    assert exc.value.key == "login"
    assert exc.value.limit == 2
    assert exc.value.retry_after > 0


def test_decorator_preserves_metadata():
    limiter = RateLimiter(limit=1, window_seconds=60)

    @limiter.limit("x")
    def my_func():
        """Docstring."""
        return 42

    assert my_func.__name__ == "my_func"
    assert my_func.__doc__ == "Docstring."


def test_decorator_dynamic_key():
    limiter = RateLimiter(limit=1, window_seconds=60)

    @limiter.limit(lambda user_id: f"user:{user_id}")
    def fetch(user_id):
        return user_id

    assert fetch(1) == 1
    assert fetch(2) == 2  # different key, independent budget
    with pytest.raises(RateLimitExceeded):
        fetch(1)


def test_decorator_on_blocked_fallback():
    limiter = RateLimiter(limit=1, window_seconds=60)

    @limiter.limit("k", on_blocked=lambda: "fallback")
    def f():
        return "real"

    assert f() == "real"
    assert f() == "fallback"


def test_rate_limit_helper():
    limiter = RateLimiter(limit=1, window_seconds=60)

    @rate_limit(limiter, "k")
    def g():
        return "ok"

    assert g() == "ok"
    with pytest.raises(RateLimitExceeded):
        g()
