"""FastAPI example using rust-py-rate-limit.

Run with:
    pip install fastapi uvicorn
    uvicorn examples.fastapi_app:app --reload

Then:
    curl -i http://127.0.0.1:8000/api/users
"""

from fastapi import FastAPI, HTTPException, Request

from rust_py_rate_limit import RateLimiter
from rust_py_rate_limit.fastapi import RateLimitMiddleware

app = FastAPI(title="rust-py-rate-limit example")

# Option A: global middleware (applies to every route).
app.add_middleware(
    RateLimitMiddleware,
    limit=100,
    window_seconds=60,
    key_func=lambda request: request.client.host if request.client else "anon",
)

# Option B: per-route manual check using a dedicated limiter.
login_limiter = RateLimiter(limit=5, window_seconds=60)


@app.get("/api/users")
def list_users():
    return {"users": []}


@app.post("/api/login")
def login(request: Request):
    key = request.client.host if request.client else "anon"
    if not login_limiter.allow(f"login:{key}"):
        raise HTTPException(status_code=429, detail="Too many login attempts")
    return {"token": "..."}
