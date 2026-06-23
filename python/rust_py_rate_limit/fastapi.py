"""FastAPI / Starlette integration.

Provides an ASGI middleware that rate-limits incoming requests and sets the
standard ``X-RateLimit-*`` and ``Retry-After`` headers.

Usage::

    from rust_py_rate_limit.fastapi import RateLimitMiddleware

    app.add_middleware(
        RateLimitMiddleware,
        limit=100,
        window_seconds=60,
        key_func=lambda request: request.client.host,
    )
"""

from __future__ import annotations

from typing import Callable, Optional

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse

from . import RateLimiter


def _default_key(request: Request) -> str:
    client = request.client
    return client.host if client is not None else "anonymous"


class RateLimitMiddleware(BaseHTTPMiddleware):
    """Starlette/FastAPI middleware applying a per-key Fixed Window limit."""

    def __init__(
        self,
        app,
        *,
        limit: int = 100,
        window_seconds: int = 60,
        key_func: Optional[Callable[[Request], str]] = None,
        limiter: Optional[RateLimiter] = None,
        detail: str = "Too many requests",
    ) -> None:
        super().__init__(app)
        self.limiter = limiter or RateLimiter(
            limit=limit, window_seconds=window_seconds
        )
        self.key_func = key_func or _default_key
        self.detail = detail

    async def dispatch(self, request: Request, call_next):
        key = self.key_func(request)
        result = self.limiter.check(key)

        if not result["allowed"]:
            response = JSONResponse(
                {"detail": self.detail}, status_code=429
            )
            response.headers["Retry-After"] = str(result["retry_after_seconds"])
        else:
            response = await call_next(request)

        response.headers["X-RateLimit-Limit"] = str(result["limit"])
        response.headers["X-RateLimit-Remaining"] = str(result["remaining"])
        response.headers["X-RateLimit-Reset"] = str(result["reset_after_seconds"])
        return response


__all__ = ["RateLimitMiddleware"]
