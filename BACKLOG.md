# BACKLOG — rust-py-rate-limit

> **Fast local rate limiting for Python, powered by Rust.**

Biblioteca de rate limiting **local ao processo**, com core em **Rust** (PyO3 + maturin), focada em velocidade, segurança e simplicidade. Instalável via PyPI.

---

## 1. Visão geral

### Objetivo
Criar uma biblioteca instalável via PyPI para rate limiting em aplicações Python, com core em Rust usando PyO3 e maturin.

### Proposta
Um rate limiter local, rápido, thread-safe e simples de usar, para proteger endpoints, funções, APIs internas e serviços Python contra excesso de requisições.

### Alvos de uso
- Python puro
- Django
- FastAPI
- Flask (futuramente)
- Scripts backend
- Workers

### Posicionamento
`rust-py-rate-limit` é uma biblioteca de rate limiting **local** para Python, com core em Rust, focada em velocidade, segurança e simplicidade.

> **Frase de produto:** "Fast local rate limiting for Python, powered by Rust."

---

## 2. Stack

| Camada | Tecnologias |
|--------|-------------|
| Core   | Rust, PyO3, maturin |
| Concorrência | DashMap, AtomicU64 |
| Tempo  | `std::time` (ou `time`) |
| Serialização | serde, serde_json |
| Erros  | thiserror |
| Python | 3.10+ |
| Testes | pytest |
| CI/CD  | GitHub Actions, TestPyPI, PyPI |

### Dependências Rust sugeridas (`Cargo.toml`)
```toml
pyo3
dashmap
serde
serde_json
thiserror
```

### Dependências Python de desenvolvimento
```txt
pytest
fastapi
uvicorn
django
httpx
```

---

## 3. Estrutura de diretórios desejada

```txt
rust-py-rate-limit/
├── Cargo.toml
├── pyproject.toml
├── README.md
├── LICENSE
├── src/
│   ├── lib.rs
│   ├── rate_limiter.rs
│   ├── fixed_window.rs
│   ├── sliding_window.rs
│   ├── token_bucket.rs
│   ├── entry.rs
│   ├── stats.rs
│   ├── errors.rs
│   └── time_utils.rs
├── python/
│   └── rust_py_rate_limit/
│       ├── __init__.py
│       ├── decorators.py
│       ├── fastapi.py
│       ├── django.py
│       └── flask.py
├── tests/
│   ├── test_fixed_window.py
│   ├── test_remaining.py
│   ├── test_reset.py
│   ├── test_stats.py
│   ├── test_decorator.py
│   ├── test_fastapi.py
│   └── test_django.py
└── examples/
    ├── basic_usage.py
    ├── fastapi_app.py
    ├── django_example/
    └── decorator_usage.py
```

---

## 4. Modelo interno (Rust)

### Entrada do Fixed Window
```rust
pub struct RateLimitEntry {
    pub count: u64,
    pub window_start: u64,
}
```

### RateLimiter
```rust
pub struct RustRateLimiter {
    pub limit: u64,
    pub window_seconds: u64,
    pub entries: DashMap<String, RateLimitEntry>,
    pub stats: RateLimitStats,
}
```

### Stats
```rust
pub struct RateLimitStats {
    pub allowed: AtomicU64,
    pub blocked: AtomicU64,
    pub total_checks: AtomicU64,
    pub keys: AtomicU64,
}
```

---

## 5. API Python (MVP)

### Construtor
```python
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=100, window_seconds=60)
```

### Métodos
```python
limiter.allow(key: str) -> bool
limiter.check(key: str) -> dict
limiter.remaining(key: str) -> int
limiter.reset(key: str) -> bool
limiter.clear() -> None
limiter.stats() -> dict
limiter.cleanup_expired() -> int
```

### Exemplo básico
```python
limiter = RateLimiter(limit=3, window_seconds=60)

assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is True
assert limiter.allow("ip:127.0.0.1") is False
```

### Retorno de `check()` — permitido
```python
{
    "allowed": True,
    "limit": 100,
    "remaining": 99,
    "reset_after_seconds": 60,
    "retry_after_seconds": 0
}
```

### Retorno de `check()` — bloqueado
```python
{
    "allowed": False,
    "limit": 100,
    "remaining": 0,
    "reset_after_seconds": 42,
    "retry_after_seconds": 42
}
```

### Retorno de `stats()`
```python
{
    "allowed": 1200,
    "blocked": 35,
    "total_checks": 1235,
    "active_keys": 20
}
```

---

## 6. Regras de comportamento

- `allow(key)` deve incrementar o contador se permitido.
- Se a chave ultrapassar o limite, deve retornar `False`.
- Quando a janela expirar, o contador da chave deve reiniciar.
- `remaining(key)` deve retornar quantas chamadas ainda restam na janela atual.
- `reset(key)` deve remover a chave.
- `clear()` deve limpar tudo.
- `cleanup_expired()` deve remover chaves com janelas expiradas.
- O rate limiter deve ser **thread-safe**.
- O rate limiter deve ser **local ao processo**.
- Em múltiplos workers, **cada worker terá seu próprio estado**.
- O MVP **não** deve prometer rate limit distribuído.
- O algoritmo inicial será **Fixed Window** por simplicidade.

### Algoritmo Fixed Window (exemplo)
```txt
limit = 3, window = 60s, key = "user:1"

request 1 -> allowed
request 2 -> allowed
request 3 -> allowed
request 4 -> blocked
após 60s -> allowed novamente
```

---

## 7. Requisitos técnicos Rust

- Usar `#[pyclass] RateLimiter`.
- Usar `#[pymethods]`.
- Usar `DashMap<String, RateLimitEntry>` para concorrência.
- Usar `AtomicU64` para estatísticas.
- **Evitar lock global** no caminho crítico.
- **Não usar `.unwrap()`** em código crítico.
- Usar `Result` quando necessário.
- Converter erros Rust para exceções Python.
- Usar `&str` quando possível.
- Separar algoritmos por módulos.
- Começar com Fixed Window. Deixar Sliding Window e Token Bucket para versões futuras.

---

## 8. Etapas de desenvolvimento (MVP)

> **Regra da mentoria:** avançar etapa por etapa, sempre com testes antes de avançar.

### Etapa 1 — Criar projeto com maturin
```bash
mkdir rust-py-rate-limit
cd rust-py-rate-limit
maturin init --bindings pyo3
```

### Etapa 2 — Configurar `Cargo.toml`
Adicionar: `pyo3`, `dashmap`, `serde`, `serde_json`, `thiserror`.

### Etapa 3 — Primeira função
```python
import rust_py_rate_limit
print(rust_py_rate_limit.hello())
```

### Etapa 4 — Criar `#[pyclass] RateLimiter`

### Etapa 5 — Criar construtor
```python
RateLimiter(limit=100, window_seconds=60)
```

### Etapa 6 — Implementar `allow(key)`
### Etapa 7 — Implementar `check(key)`
### Etapa 8 — Implementar `remaining(key)`
### Etapa 9 — Implementar `reset(key)`
### Etapa 10 — Implementar `clear()`
### Etapa 11 — Implementar `stats()`
### Etapa 12 — Implementar `cleanup_expired()`

### Etapa 13 — Criar testes com pytest
Testes obrigatórios:
1. Permitir até o limite.
2. Bloquear após o limite.
3. Reiniciar após expiração da janela.
4. `remaining()` antes e depois de chamadas.
5. `check()` permitido.
6. `check()` bloqueado.
7. `reset()` remove chave.
8. `clear()` remove tudo.
9. `stats()` conta allowed.
10. `stats()` conta blocked.
11. Várias chaves independentes.
12. Chave expirada é limpa.
13. Concorrência com threads Python.
14. Parâmetros inválidos.
15. Limite zero deve retornar erro ou bloquear tudo (definir comportamento).

Exemplo:
```python
import time
from rust_py_rate_limit import RateLimiter

def test_blocks_after_limit():
    limiter = RateLimiter(limit=2, window_seconds=60)
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False

def test_resets_after_window():
    limiter = RateLimiter(limit=1, window_seconds=1)
    assert limiter.allow("user:1") is True
    assert limiter.allow("user:1") is False
    time.sleep(1.1)
    assert limiter.allow("user:1") is True
```

### Etapa 14 — Decorator Python
```python
limiter = RateLimiter(limit=5, window_seconds=60)

@limiter.limit("login")
def login():
    return "ok"
```
Se bloqueado, lançar `RateLimitExceeded`.

### Etapa 15 — FastAPI middleware
```python
from rust_py_rate_limit.fastapi import RateLimitMiddleware

app.add_middleware(
    RateLimitMiddleware,
    limit=100,
    window_seconds=60,
    key_func=lambda request: request.client.host,
)
```
Resposta bloqueada: `{"detail": "Too many requests"}` com status `429`.

Headers desejados:
```txt
X-RateLimit-Limit
X-RateLimit-Remaining
X-RateLimit-Reset
Retry-After
```

### Etapa 16 — Django middleware
```python
MIDDLEWARE = [
    "rust_py_rate_limit.django.RateLimitMiddleware",
]
```
```python
# settings.py
RUST_PY_RATE_LIMIT = {
    "LIMIT": 100,
    "WINDOW_SECONDS": 60,
    "KEY": "ip",
}
```

### Etapa 17 — Criar README (detalhado, em inglês)
**Entregar um README completo e detalhado, escrito em inglês**, com toda a documentação do projeto.

Deve conter (todas as seções em inglês):
- What is the project
- Why Rust
- Installation (`pip install rust-py-rate-limit`)
- Quick start / basic usage
- Fixed Window algorithm explained
- API reference (`allow`, `check`, `remaining`, `reset`, `clear`, `stats`, `cleanup_expired`) com tabelas de parâmetros e retornos
- FastAPI usage + middleware
- Django usage + middleware
- Decorator usage
- Stats
- Limitations (honest section)
- Roadmap
- License

Seção de limitações (em inglês):
- Rate limit cache is local to the process.
- On Gunicorn/Uvicorn with multiple workers, each worker has its own counter.
- It does not replace Redis for distributed rate limiting.
- Fixed Window may allow bursts at the boundary between windows.
- For distributed production use, use a Redis/Postgres backend in the future.

### Etapa 18 — GitHub Actions
- `cargo test`
- `pytest`
- `maturin build`
- wheels multiplataforma

### Etapa 19 — Publicar no TestPyPI

### Etapa 20 — Publicar no PyPI com versão `0.1.0`

---

## 9. Exemplos de uso (referência)

### FastAPI (manual)
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

### Django (manual)
```python
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=100, window_seconds=60)

def my_view(request):
    key = request.META.get("REMOTE_ADDR")
    if not limiter.allow(key):
        return JsonResponse({"detail": "Too many requests"}, status=429)
    return JsonResponse({"ok": True})
```

### Decorator
```python
from rust_py_rate_limit import RateLimiter

limiter = RateLimiter(limit=5, window_seconds=60)

@limiter.limit("login")
def login():
    return "ok"
```

---

## 10. Roadmap de versões

### v0.1.0 (MVP)
- Fixed Window
- `allow`, `check`, `remaining`, `reset`, `stats`
- pytest + README

### v0.2.0
- Decorator
- FastAPI middleware
- Django middleware
- Headers HTTP

### v0.3.0
- Sliding Window
- Token Bucket
- Cleanup background

### v0.4.0
- Redis backend
- Distributed rate limit

### v0.5.0
- Prometheus
- Integração com ImmutableLog

---

## 11. Funcionalidades futuras

- Sliding Window
- Token Bucket
- Leaky Bucket
- Rate limit por IP / usuário / API Key
- Decorator
- FastAPI / Django / Flask middleware
- Redis backend (futuro)
- Postgres backend (futuro)
- Distributed rate limit
- Headers HTTP automáticos
- Métricas Prometheus
- Integração com ImmutableLog para eventos de bloqueio
- Bloqueio temporário de chave abusiva
- Whitelist / Blacklist

### Integração futura com ImmutableLog
Quando uma chave for bloqueada muitas vezes, enviar evento:
```python
{
    "event_type": "rate_limit_blocked",
    "key": "ip:127.0.0.1",
    "limit": 100,
    "window_seconds": 60,
    "blocked_at": "timestamp"
}
```

---

## 12. Website (landing page)

Entregar um **website** (landing page de documentação/divulgação) no diretório:

```txt
/Users/robertolima/Documents/projects/rust/study/rust_py_rate_limit_website
```

### Referência
Basear-se no site de referência já existente:

```txt
/Users/robertolima/Documents/projects/rust/study/rust_py_audit_website
```

Replicar a mesma stack e estrutura (site **estático**, deploy na Vercel):
- `index.html` — landing page principal
- `style.css` — estilos
- `script.js` — interações
- `favicon.ico` / `favicon.png`
- `vercel.json` — configuração de deploy
- `.gitignore`

### Conteúdo do site (em inglês, alinhado ao README)
- Hero com a frase de produto: *"Fast local rate limiting for Python, powered by Rust."*
- O que é / por que Rust
- Instalação (`pip install rust-py-rate-limit`)
- Exemplos de uso (básico, FastAPI, Django, decorator)
- API reference resumida
- Stats
- Limitações (honestas)
- Roadmap
- Links (PyPI, GitHub, licença)

> Adaptar branding, textos e exemplos de código de `rust_py_audit` para `rust-py-rate-limit`, mantendo o layout, o design e a configuração de deploy da referência.

---

## 13. Definition of Done (MVP / v0.1.0)

- [ ] Projeto criado com maturin + PyO3, build local funcionando.
- [ ] `hello()` exposto e testado.
- [ ] `#[pyclass] RateLimiter` com construtor `(limit, window_seconds)`.
- [ ] `allow`, `check`, `remaining`, `reset`, `clear`, `stats`, `cleanup_expired` implementados.
- [ ] Algoritmo Fixed Window completo e correto.
- [ ] Thread-safety garantido (DashMap + AtomicU64, sem lock global no caminho crítico).
- [ ] Erros Rust convertidos para exceções Python.
- [ ] Suite pytest com os 15 testes obrigatórios passando.
- [ ] README detalhado **em inglês** com toda a documentação e seção de limitações honesta.
- [ ] Website estático entregue em `rust_py_rate_limit_website`, baseado na referência `rust_py_audit_website`.
- [ ] GitHub Actions (cargo test + pytest + maturin build).
- [ ] Publicado no TestPyPI e validado.
- [ ] Publicado no PyPI como `0.1.0`.

Resultado final esperado:
```bash
pip install rust-py-rate-limit
```
```python
from rust_py_rate_limit import RateLimiter
```

