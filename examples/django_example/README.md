# Django example

Minimal wiring to use `rust-py-rate-limit` in a Django project.

## 1. Install

```bash
pip install rust-py-rate-limit django
```

## 2. Configure `settings.py`

```python
MIDDLEWARE = [
    # ... other middleware ...
    "rust_py_rate_limit.django.RateLimitMiddleware",
]

RUST_PY_RATE_LIMIT = {
    "LIMIT": 100,
    "WINDOW_SECONDS": 60,
    "KEY": "ip",  # "ip" or "user"
}
```

The middleware adds `X-RateLimit-*` headers to every response and returns
`429 {"detail": "Too many requests"}` (with a `Retry-After` header) when a
client exceeds the limit.

## 3. Or check manually in a view

```python
from django.http import JsonResponse
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=100, window_seconds=60)


def my_view(request):
    key = request.META.get("REMOTE_ADDR")
    if not limiter.allow(key):
        return JsonResponse({"detail": "Too many requests"}, status=429)
    return JsonResponse({"ok": True})
```

> Remember: state is per-process. Behind multiple workers each worker has its
> own counters.
