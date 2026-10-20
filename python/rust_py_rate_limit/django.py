"""Django integration.

Add to ``settings.py``::

    MIDDLEWARE = [
        # ...
        "rust_py_rate_limit.django.RateLimitMiddleware",
    ]

    RUST_PY_RATE_LIMIT = {
        "LIMIT": 100,
        "WINDOW_SECONDS": 60,
        "KEY": "ip",  # "ip" or "user"
        "MAX_KEYS": 100_000,  # optional: bound memory under a key flood
        "CLEANUP_INTERVAL": 60,  # optional: drop expired keys in the background
    }
"""

from __future__ import annotations

from django.conf import settings
from django.http import JsonResponse

from . import RateLimiter


class RateLimitMiddleware:
    """Django middleware applying a per-key Fixed Window limit.

    Reads its configuration from the ``RUST_PY_RATE_LIMIT`` setting.
    """

    def __init__(self, get_response):
        self.get_response = get_response

        config = getattr(settings, "RUST_PY_RATE_LIMIT", {}) or {}
        self.limit = int(config.get("LIMIT", 100))
        self.window_seconds = int(config.get("WINDOW_SECONDS", 60))
        self.key_type = str(config.get("KEY", "ip"))
        self.detail = str(config.get("DETAIL", "Too many requests"))
        max_keys = config.get("MAX_KEYS")
        cleanup_interval = config.get("CLEANUP_INTERVAL")

        self.limiter = RateLimiter(
            limit=self.limit,
            window_seconds=self.window_seconds,
            max_keys=int(max_keys) if max_keys is not None else None,
            cleanup_interval=(
                float(cleanup_interval) if cleanup_interval is not None else None
            ),
        )

    def _resolve_key(self, request) -> str:
        if self.key_type == "user":
            user = getattr(request, "user", None)
            if user is not None and getattr(user, "is_authenticated", False):
                return f"user:{user.pk}"
        return request.META.get("REMOTE_ADDR", "anonymous")

    def __call__(self, request):
        key = self._resolve_key(request)
        result = self.limiter.check(key)

        if not result["allowed"]:
            response = JsonResponse({"detail": self.detail}, status=429)
            response["Retry-After"] = str(result["retry_after_seconds"])
        else:
            response = self.get_response(request)

        response["X-RateLimit-Limit"] = str(result["limit"])
        response["X-RateLimit-Remaining"] = str(result["remaining"])
        response["X-RateLimit-Reset"] = str(result["reset_after_seconds"])
        return response


__all__ = ["RateLimitMiddleware"]
