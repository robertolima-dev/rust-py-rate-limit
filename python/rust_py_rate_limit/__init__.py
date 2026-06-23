"""rust_py_rate_limit — Fast local rate limiting for Python, powered by Rust.

The hot path (``allow``/``check``/``remaining``/...) is implemented in Rust and
exposed through the compiled ``_core`` extension. This module subclasses the
native :class:`RateLimiter` to add the ergonomic ``.limit(...)`` decorator while
keeping every core method native.

Basic usage::

    from rust_py_rate_limit import RateLimiter

    limiter = RateLimiter(limit=10, window_seconds=60)
    if limiter.allow("user:123"):
        ...  # allowed
    else:
        ...  # blocked
"""

from __future__ import annotations

from typing import Callable, Optional

from ._core import RateLimiter as _RateLimiter
from ._core import __version__, hello
from .decorators import KeySpec, rate_limit
from .exceptions import RateLimitExceeded


class RateLimiter(_RateLimiter):
    """Fixed Window rate limiter.

    Inherits the native methods ``allow``, ``check``, ``remaining``, ``reset``,
    ``clear``, ``stats`` and ``cleanup_expired`` from the Rust core, and adds
    the :meth:`limit` decorator.
    """

    def limit(
        self,
        key: KeySpec,
        *,
        on_blocked: Optional[Callable[..., object]] = None,
    ):
        """Decorator that rate-limits a callable using this limiter.

        ::

            limiter = RateLimiter(limit=5, window_seconds=60)

            @limiter.limit("login")
            def login():
                return "ok"

        ``key`` may also be a callable that derives the key from the wrapped
        function's arguments. When blocked, :class:`RateLimitExceeded` is raised
        unless ``on_blocked`` is provided.
        """
        return rate_limit(self, key, on_blocked=on_blocked)


__all__ = [
    "RateLimiter",
    "RateLimitExceeded",
    "rate_limit",
    "hello",
    "__version__",
]
