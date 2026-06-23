"""Flask integration (preview).

Flask support is on the roadmap; this module already ships a working helper for
decorating views::

    from flask import Flask
    from rust_py_rate_limit.flask import FlaskRateLimiter

    app = Flask(__name__)
    limiter = FlaskRateLimiter(app, limit=100, window_seconds=60)

    @app.get("/api/users")
    @limiter.limit()
    def list_users():
        return {"users": []}
"""

from __future__ import annotations

from functools import wraps
from typing import Callable, Optional, Union

from flask import jsonify, request

from . import RateLimiter

KeySpec = Union[str, Callable[[], str]]


class FlaskRateLimiter:
    """Per-view Fixed Window rate limiting for Flask."""

    def __init__(
        self,
        app=None,
        *,
        limit: int = 100,
        window_seconds: int = 60,
        key_func: Optional[Callable[[], str]] = None,
        detail: str = "Too many requests",
    ) -> None:
        self.limiter = RateLimiter(limit=limit, window_seconds=window_seconds)
        self.key_func = key_func or (lambda: request.remote_addr or "anonymous")
        self.detail = detail
        if app is not None:
            self.init_app(app)

    def init_app(self, app) -> None:
        self._app = app

    def limit(self, key: Optional[KeySpec] = None):
        """Decorate a view, returning HTTP 429 when the limit is exceeded."""

        def decorator(func: Callable) -> Callable:
            @wraps(func)
            def wrapper(*args, **kwargs):
                if key is None:
                    resolved = self.key_func()
                elif callable(key):
                    resolved = key()
                else:
                    resolved = key

                result = self.limiter.check(resolved)
                if not result["allowed"]:
                    response = jsonify({"detail": self.detail})
                    response.status_code = 429
                    response.headers["Retry-After"] = str(
                        result["retry_after_seconds"]
                    )
                    response.headers["X-RateLimit-Limit"] = str(result["limit"])
                    response.headers["X-RateLimit-Remaining"] = str(
                        result["remaining"]
                    )
                    response.headers["X-RateLimit-Reset"] = str(
                        result["reset_after_seconds"]
                    )
                    return response
                return func(*args, **kwargs)

            return wrapper

        return decorator


__all__ = ["FlaskRateLimiter"]
