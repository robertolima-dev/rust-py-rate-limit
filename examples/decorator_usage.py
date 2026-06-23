"""Decorator usage of rust-py-rate-limit.

Run with:  python examples/decorator_usage.py
"""

from rust_py_rate_limit import RateLimiter, RateLimitExceeded

limiter = RateLimiter(limit=2, window_seconds=60)


@limiter.limit("login")
def login() -> str:
    return "ok"


# Dynamic key derived from the function arguments.
@limiter.limit(lambda user_id: f"user:{user_id}")
def fetch_profile(user_id: int) -> dict:
    return {"id": user_id}


def main() -> None:
    print("login:", login())
    print("login:", login())
    try:
        login()
    except RateLimitExceeded as exc:
        print("blocked:", exc, "| retry_after:", exc.retry_after)

    print("fetch 1:", fetch_profile(1))
    print("fetch 2:", fetch_profile(2))  # different key, own budget


if __name__ == "__main__":
    main()
