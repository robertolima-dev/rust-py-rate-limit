"""Django middleware integration tests."""

import pytest

django = pytest.importorskip("django")

from django.conf import settings  # noqa: E402


def _configure_django(limit=2, window_seconds=60, key="ip"):
    if not settings.configured:
        settings.configure(
            DEBUG=True,
            ALLOWED_HOSTS=["*"],
            DATABASES={},
            INSTALLED_APPS=[],
            RUST_PY_RATE_LIMIT={
                "LIMIT": limit,
                "WINDOW_SECONDS": window_seconds,
                "KEY": key,
            },
        )
        django.setup()


def test_django_middleware_blocks_after_limit():
    _configure_django(limit=2)

    from django.http import JsonResponse  # noqa: E402
    from django.test import RequestFactory  # noqa: E402

    from rust_py_rate_limit.django import RateLimitMiddleware  # noqa: E402

    def get_response(request):
        return JsonResponse({"ok": True})

    middleware = RateLimitMiddleware(get_response)
    factory = RequestFactory()

    def call():
        request = factory.get("/ping", REMOTE_ADDR="1.2.3.4")
        return middleware(request)

    r1 = call()
    assert r1.status_code == 200
    assert r1["X-RateLimit-Limit"] == "2"
    assert r1["X-RateLimit-Remaining"] == "1"

    r2 = call()
    assert r2.status_code == 200
    assert r2["X-RateLimit-Remaining"] == "0"

    r3 = call()
    assert r3.status_code == 429
    assert r3["Retry-After"]
