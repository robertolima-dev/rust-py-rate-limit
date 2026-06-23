"""Decorator helpers for rate limiting plain callables."""

from __future__ import annotations

import functools
from typing import Callable, Optional, Union

from .exceptions import RateLimitExceeded

# A key is either a fixed string or a callable that derives one from the
# wrapped function's arguments.
KeySpec = Union[str, Callable[..., str]]


def rate_limit(
    limiter,
    key: KeySpec,
    *,
    on_blocked: Optional[Callable[..., object]] = None,
):
    """Return a decorator that rate-limits the wrapped callable.

    Parameters
    ----------
    limiter:
        A :class:`~rust_py_rate_limit.RateLimiter` instance.
    key:
        Either a fixed key string, or a callable receiving the wrapped
        function's ``*args, **kwargs`` and returning a key string.
    on_blocked:
        Optional fallback called (with the same args) when the request is
        blocked. If omitted, :class:`RateLimitExceeded` is raised instead.
    """

    def decorator(func: Callable) -> Callable:
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            resolved = key(*args, **kwargs) if callable(key) else key
            result = limiter.check(resolved)
            if not result["allowed"]:
                if on_blocked is not None:
                    return on_blocked(*args, **kwargs)
                raise RateLimitExceeded(
                    resolved,
                    limit=result["limit"],
                    retry_after=result["retry_after_seconds"],
                )
            return func(*args, **kwargs)

        return wrapper

    return decorator
