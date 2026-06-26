# rust-py-rate-limit

> **Fast local rate limiting for Python, powered by Rust.**

🌐 **Website:** [rust-py-rate-limit.vercel.app](https://rust-py-rate-limit.vercel.app/)

A fast, thread-safe, in-process rate limiter for Python with a core written in
**Rust** (via [PyO3](https://pyo3.rs) + [maturin](https://www.maturin.rs)). Use
it to protect endpoints, functions, internal APIs, workers and backend scripts
against bursts of traffic — with zero external services.

```python
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=10, window_seconds=60)

if limiter.allow("user:123"):
    print("allowed")
else:
    print("blocked")
```

---

## Table of contents

- [What is this?](#what-is-this)
- [Why Rust?](#why-rust)
- [Installation](#installation)
- [Quick start](#quick-start)
- [How Fixed Window works](#how-fixed-window-works)
- [How Sliding Window works](#how-sliding-window-works)
- [API reference](#api-reference)
- [FastAPI](#fastapi)
- [Django](#django)
- [Flask](#flask)
- [Decorator](#decorator)
- [Statistics](#statistics)
- [Limitations](#limitations)
- [Roadmap](#roadmap)
- [Development](#development)
- [License](#license)

---

## What is this?

`rust-py-rate-limit` is a **local** (in-process) rate limiter. Every limiter
instance keeps its counters in memory inside your Python process, guarded by a
concurrent, sharded hash map on the Rust side. There is no Redis, no network
hop, and no serialization on the hot path — just a couple of atomic operations
per request.

It works anywhere Python runs:

- Plain Python
- FastAPI
- Django
- Flask (preview)
- Background workers and scripts

## Why Rust?

- **Speed** — the counting logic is compiled native code; the hot path releases
  the GIL so multiple Python threads can check limits in parallel.
- **Safety** — no data races by construction. State lives in a
  [`DashMap`](https://docs.rs/dashmap) (a sharded concurrent map) and statistics
  use lock-free atomics, so there is no global lock on the critical path.
- **Simplicity** — a tiny, predictable API surface that is hard to misuse.

## Installation

```bash
pip install rust-py-rate-limit
```

Requires Python 3.10+. Wheels are published for Linux, macOS and Windows, so no
Rust toolchain is needed to install.

## Quick start

```python
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=3, window_seconds=60)

assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is False   # limit reached
```

Opt into the **Sliding Window** algorithm to smooth bursts at the window
boundary (the default is `"fixed"`):

```python
limiter = RateLimiter(limit=100, window_seconds=60, algorithm="sliding")
limiter.algorithm  # "sliding"
```

## How Fixed Window works

The default algorithm is **Fixed Window**. Each key gets a counter and a window
start time. Within a window of `window_seconds`, up to `limit` requests are
admitted; once the window elapses, the counter resets.

```text
limit = 3, window = 60s, key = "user:1"

request 1 -> allowed
request 2 -> allowed
request 3 -> allowed
request 4 -> blocked
... 60s later ...
request 5 -> allowed   (new window)
```

Fixed Window is simple and cheap. Its only caveat is that it can admit up to
`2 * limit` requests around a window boundary (a burst at the end of one window
plus a burst at the start of the next). If you need stricter smoothing, use
`algorithm="sliding"` (below).

## How Sliding Window works

Set `algorithm="sliding"` for the **sliding window counter**, which removes the
boundary doubling. It keeps the count for the current aligned window plus the
previous one, and weights the previous window by how much of it still overlaps
the trailing `window_seconds` ending at *now*:

```text
estimated = previous_count * weight + current_count
weight    = (window_seconds - elapsed_in_current_window) / window_seconds
```

A request is admitted while `estimated < limit`. This is O(1) in time and memory
per key (unlike a sliding *log*, which stores every timestamp) and smooths the
burst at the boundary, at the cost of being an approximation rather than an exact
count.

```python
limiter = RateLimiter(limit=10, window_seconds=60, algorithm="sliding")
# A full burst at the end of one window leaves far fewer slots right after the
# boundary, instead of a fresh `limit` as Fixed Window would.
```

## API reference

```python
RateLimiter(limit: int, window_seconds: int, algorithm: str = "fixed")
```

`limit` and `window_seconds` must be **positive integers** (passing `0` or a
negative value raises `ValueError`). `algorithm` selects the strategy —
`"fixed"` (default) or `"sliding"`; any other value raises `ValueError`.

| Method | Returns | Description |
| --- | --- | --- |
| `allow(key: str)` | `bool` | Consume one request. `True` if admitted, `False` if blocked. |
| `check(key: str)` | `dict` | Consume one request and return full detail (see below). |
| `remaining(key: str)` | `int` | Requests left in the current window **without** consuming one. |
| `reset(key: str)` | `bool` | Drop a key's state. `True` if it existed. |
| `clear()` | `None` | Drop all keys. |
| `stats()` | `dict` | Activity counters (see [Statistics](#statistics)). |
| `cleanup_expired()` | `int` | Remove keys whose window has expired. Returns the count removed. |

Read-only properties: `limiter.max_requests`, `limiter.window_seconds` and
`limiter.algorithm` (`"fixed"` or `"sliding"`). (The configured limit is
`max_requests`, since `.limit(...)` is the decorator.)

### `check()` return value

Allowed:

```python
{
    "allowed": True,
    "limit": 100,
    "remaining": 99,
    "reset_after_seconds": 60,
    "retry_after_seconds": 0,
}
```

Blocked:

```python
{
    "allowed": False,
    "limit": 100,
    "remaining": 0,
    "reset_after_seconds": 42,
    "retry_after_seconds": 42,
}
```

## FastAPI

### Manual check

```python
from fastapi import FastAPI, Request, HTTPException
from rust_py_rate_limit import RateLimiter

app = FastAPI()
limiter = RateLimiter(limit=100, window_seconds=60)

@app.get("/api/users")
def list_users(request: Request):
    key = request.client.host
    if not limiter.allow(key):
        raise HTTPException(status_code=429, detail="Too many requests")
    return {"users": []}
```

### Middleware

```python
from rust_py_rate_limit.fastapi import RateLimitMiddleware

app.add_middleware(
    RateLimitMiddleware,
    limit=100,
    window_seconds=60,
    key_func=lambda request: request.client.host,
)
```

When a request is blocked the middleware responds with `429` and
`{"detail": "Too many requests"}`. Every response carries the standard headers:

```text
X-RateLimit-Limit
X-RateLimit-Remaining
X-RateLimit-Reset
Retry-After      (only when blocked)
```

## Django

```python
# settings.py
MIDDLEWARE = [
    # ...
    "rust_py_rate_limit.django.RateLimitMiddleware",
]

RUST_PY_RATE_LIMIT = {
    "LIMIT": 100,
    "WINDOW_SECONDS": 60,
    "KEY": "ip",  # "ip" or "user"
}
```

Or check manually in a view:

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

## Flask

```python
from flask import Flask
from rust_py_rate_limit.flask import FlaskRateLimiter

app = Flask(__name__)
limiter = FlaskRateLimiter(app, limit=100, window_seconds=60)

@app.get("/api/users")
@limiter.limit()
def list_users():
    return {"users": []}
```

## Decorator

```python
from rust_py_rate_limit import RateLimiter, RateLimitExceeded

limiter = RateLimiter(limit=5, window_seconds=60)

@limiter.limit("login")
def login():
    return "ok"
```

When the limit is exceeded the decorated function raises `RateLimitExceeded`
(which carries `.key`, `.limit` and `.retry_after`). The key may also be a
callable that derives the key from the function's arguments:

```python
@limiter.limit(lambda user_id: f"user:{user_id}")
def fetch(user_id):
    ...
```

## Statistics

```python
limiter.stats()
# {
#     "allowed": 1200,
#     "blocked": 35,
#     "total_checks": 1235,
#     "active_keys": 20,
# }
```

## Limitations

Be honest with yourself about what an in-process limiter can and cannot do:

- The rate-limit state is **local to the process**.
- Under Gunicorn/Uvicorn with **multiple workers**, each worker keeps its own
  counters, so the effective global limit is roughly `limit × workers`.
- It is **not** a replacement for Redis when you need distributed rate limiting.
- Fixed Window can allow short bursts at the boundary between two windows; use
  `algorithm="sliding"` to smooth them.
- For distributed production setups, a Redis/Postgres backend is planned (see
  the roadmap).

## Roadmap

| Version | Highlights | Status |
| --- | --- | --- |
| **v0.1.0** | Fixed Window · `allow`/`check`/`remaining`/`reset`/`clear`/`stats`/`cleanup_expired` · pytest · README | ✅ |
| **v0.1.5** | Decorator · FastAPI/Django/Flask middleware · HTTP headers | ✅ |
| **v0.2.0** | Sliding Window (`algorithm="sliding"`) | ✅ |
| v0.3.0 | Token Bucket · background cleanup | 🔜 |
| v0.4.0 | Redis backend · distributed rate limiting | 🔜 |
| v0.5.0 | Prometheus metrics · ImmutableLog integration | 🔜 |

## Development

```bash
# Rust unit tests
cargo test

# Build the extension into a virtualenv and run the Python tests
python -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"      # or: pip install maturin && maturin develop
maturin develop
pytest
```

## License

[MIT](LICENSE) © Roberto Lima
