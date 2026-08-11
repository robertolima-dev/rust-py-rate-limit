# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
e o versionamento segue [SemVer](https://semver.org/lang/pt-BR/).

## [0.2.1] - 2026-08-11

Release de empacotamento: nada muda no comportamento da biblioteca, mas muda
bastante **quem consegue instalá-la sem compilar**.

### Corrigido

- **Cobertura de plataformas.** A 0.2.0 publicou **3 wheels, todas `cp312`**:
  `macosx_11_0_arm64`, `manylinux_2_34_x86_64` e `win_amd64`. Na prática,
  caíam no sdist — ou seja, precisavam de toolchain Rust para instalar:
  - quem está em Python 3.10, 3.11, 3.13 ou 3.14 (só a 3.12 tinha wheel);
  - Linux aarch64;
  - Alpine / musl, qualquer arquitetura;
  - Mac Intel;
  - glibc anterior à 2.34 — o build saía do runner sem passar pelo container
    manylinux, então herdava a glibc da máquina em vez de declarar um piso
    portável.

  Agora são **7 wheels** (2 manylinux, 2 musllinux, 1 Windows, 2 macOS) mais o
  sdist, e o Linux volta a sair do container manylinux.
- **A publicação não roda mais sem teste.** O `CI.yml` (fmt, clippy,
  `cargo test`, pytest em 3.10–3.13) é um workflow **separado**: um push de tag
  ia direto para build e publicação sem depender dele. O `release.yml` agora
  tem o seu próprio job `test` e todos os demais declaram `needs: test`.
  (BACKLOG2 P1-1)
- **O upload aguenta o 500 do PyPI.** O upload é arquivo por arquivo e sem
  transação — uma falha no meio deixa a versão publicada pela metade, e o PyPI
  nunca permite reusar um nome de arquivo. Agora há `skip-existing` (que é o
  que torna a segunda tentativa segura), uma segunda tentativa explícita,
  conferência do conjunto de artefatos **antes** de publicar e validação
  contra a API do PyPI **depois**. (BACKLOG2 P1-9)

### Alterado

- **Wheels `abi3-py310`** (BACKLOG2 P1-2). Uma wheel por plataforma cobre
  CPython 3.10 e toda versão posterior, em vez de uma por interpretador.
  Quando sair um Python novo, a wheel existente já funciona — antes era
  preciso cortar um release só para isso.
  - **Contrapartida:** wheels `abi3` usam a Limited API, que em alguns padrões
    de chamada é marginalmente mais lenta que uma wheel específica de versão.
    Para esta biblioteca a troca compensa com folga: o custo é pequeno e o
    ganho é sair de "uma versão de Python atendida" para "todas".
  - PyPy não é afetado: a 0.2.0 já não publicava wheels PyPy, e `abi3` é
    mecanismo só de CPython.

### Adicionado

- Este `CHANGELOG.md`. (BACKLOG2 P1-5)

## [0.2.0] e anteriores

Sem changelog retroativo. O histórico está no `BACKLOG.md` e no `git log`.

Resumo do que o projeto entrega hoje:

- `RateLimiter` com janela deslizante, core em Rust sobre `DashMap`.
- `allow_threads` em todos os métodos — duas threads Python entram no core ao
  mesmo tempo.
- Middlewares para FastAPI, Django e Flask.
- Versão de fonte única: `__version__` vem de `env!("CARGO_PKG_VERSION")`.

### Limitação conhecida (não corrigida nesta versão)

⚠️ **Memória sem teto.** `cleanup_expired()` existe mas é manual: não há
`max_keys` nem varredura automática. Como o caso de uso central é rate limit
por IP ou por token, quem chama controla as chaves — um cliente hostil gera
chaves infinitas e o mapa cresce até derrubar o processo. Ver **P2-3** no
`BACKLOG2.md`.
