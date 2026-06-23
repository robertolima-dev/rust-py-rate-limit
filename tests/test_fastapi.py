"""FastAPI middleware integration tests."""

import pytest

fastapi = pytest.importorskip("fastapi")
pytest.importorskip("httpx")
from fastapi import FastAPI  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402

from rust_py_rate_limit.fastapi import RateLimitMiddleware  # noqa: E402


def build_app(limit=2, window_seconds=60):
    app = FastAPI()
    app.add_middleware(
        RateLimitMiddleware,
        limit=limit,
        window_seconds=window_seconds,
        # Stable key so the test does not depend on the test client's host.
        key_func=lambda request: "test-client",
    )

    @app.get("/ping")
    def ping():
        return {"pong": True}

    return app


def test_allows_until_limit_then_429():
    client = TestClient(build_app(limit=2))

    r1 = client.get("/ping")
    assert r1.status_code == 200
    assert r1.headers["X-RateLimit-Limit"] == "2"
    assert r1.headers["X-RateLimit-Remaining"] == "1"

    r2 = client.get("/ping")
    assert r2.status_code == 200
    assert r2.headers["X-RateLimit-Remaining"] == "0"

    r3 = client.get("/ping")
    assert r3.status_code == 429
    assert r3.json() == {"detail": "Too many requests"}
    assert "Retry-After" in r3.headers


def test_headers_present_on_success():
    client = TestClient(build_app(limit=5))
    r = client.get("/ping")
    assert r.status_code == 200
    for header in ("X-RateLimit-Limit", "X-RateLimit-Remaining", "X-RateLimit-Reset"):
        assert header in r.headers
