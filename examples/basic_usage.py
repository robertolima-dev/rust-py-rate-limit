"""Basic usage of rust-py-rate-limit.

Run with:  python examples/basic_usage.py
"""

from rust_py_rate_limit import RateLimiter


def main() -> None:
    limiter = RateLimiter(limit=3, window_seconds=60)
    key = "ip:127.0.0.1"

    for i in range(1, 6):
        allowed = limiter.allow(key)
        print(f"request {i}: {'allowed' if allowed else 'blocked'}")

    print("\ncheck():", limiter.check(key))
    print("remaining(other):", limiter.remaining("ip:10.0.0.1"))
    print("stats():", limiter.stats())


if __name__ == "__main__":
    main()
