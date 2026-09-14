# task-dashboard-IA

Coordinación entre devs y sus agentes sobre un mismo repo, para cualquier proyecto: reserva atómica de IDs, claims de
fichas (quién tiene qué AHORA) y sugerencias sueltas — lo que el markdown del repo no puede
representar. El estado de las fichas sigue viviendo en el repo del proyecto; esto lo **lee**, no lo
copia. **Un dato, un dueño.**

## Estado

Paso 1 en curso: `apps/api` responde `/health`; falta la base, la auth y las herramientas MCP.

- [docs/CONTEXTO-INICIAL.md](docs/CONTEXTO-INICIAL.md) — el problema, la lección de diseño y las
  decisiones tomadas. Punto de partida histórico, no estado.
- [docs/PLAN-PASO-1.md](docs/PLAN-PASO-1.md) — backend + MCP-vivo: esquema, contrato de
  herramientas y pasos con verificación.
- [docs/TODO.md](docs/TODO.md) — **fuente de verdad del estado**: lo abierto y lo hecho.
- [docs/GOTCHAS.md](docs/GOTCHAS.md) — trampas verificadas del repo y su stack.
- [docs/DECISIONS/](docs/DECISIONS/README.md) — ADRs; vacío hasta la primera decisión cerrada.
- `.claude/rules/00-chasis.md` — la regla siempre-cargada del agente; skills `/td-plan`,
  `/td-dev`, `/td-learn` en `.claude/skills/`.

## Forma (decidida, ver contexto)

Ramas: `develop` para trabajar, `main` es lo desplegado (el merge deploya en Railway).

Un solo servicio en **Railway** (backend + MCP por HTTP + indexador + frontend en un proceso) y un
**Postgres** gestionado (1 vCPU / 1 GB). Identidad: **syntroAuth** (JWT + JWKS); esta app autoriza
(proyectos, membresía, PATs para el MCP). Lenguaje: Rust. Fecha del paso 1: **2026-09-30**.

Layout del monorepo (workspace de Cargo; lo que no existe se crea cuando entra el código):

```
apps/api/    # Rust: axum (+ rmcp y sqlx cuando lleguen). Sirve /health, /api, /mcp.
apps/web/    # frontend del dashboard (paso 3) — no existe todavía
db/          # migraciones sqlx (paso 2) — no existe todavía
docs/        # bóveda: contexto, plan, TODO, gotchas, decisiones
scripts/     # gates del chasis (vendored de chasis-kit; no se re-estilan acá)
```

Correr en local:

```bash
cargo run -p task-dashboard-api
```

`curl localhost:8080/health`. Con Docker: `docker build -t td .` y `docker run -p 8080:8080 td`.

## Deploy

Railway construye el `Dockerfile` y despliega **solo cuando `main` cambia** (repo conectado al
servicio; `railway.json` fija el healthcheck en `/health`). No hay paso de deploy en CI: la CI
(`.github/workflows/ci.yml`) es el gate que corre antes, en cada PR hacia `main`.

## Cómo se trabaja

Al clonar, **una vez**:

```bash
make hooks
```

Cablea `.githooks/pre-commit` en este clon (`core.hooksPath` es local y no viaja). Sin eso, el
commit no corre ninguna red. El gate completo es `make gate`; mientras no exista `Cargo.toml` corre
solo los gates de Node y lo dice.
