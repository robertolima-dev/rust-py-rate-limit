"""Exceptions raised by ``rust_py_rate_limit``."""

from __future__ import annotations

from typing import Optional


class RateLimitExceeded(Exception):
    """Raised when a rate-limited callable exceeds its limit.

    Attributes
    ----------
    key:
        The rate-limit key that was exceeded.
    limit:
        The configured request limit for the window (if known).
    retry_after:
        Seconds to wait before the key is admitted again (if known).
    """

    def __init__(
        self,
        key: str,
        *,
        limit: Optional[int] = None,
        retry_after: Optional[int] = None,
    ) -> None:
        self.key = key
        self.limit = limit
        self.retry_after = retry_after

        message = f"Rate limit exceeded for key={key!r}"
        if limit is not None:
            message += f" (limit={limit})"
        if retry_after is not None:
            message += f"; retry after {retry_after}s"
        super().__init__(message)
