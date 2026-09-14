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
apps/api/    # Rust: axum + sqlx (+ rmcp cuando llegue). Sirve /health, /api, /mcp.
apps/web/    # frontend del dashboard (paso 3) — no existe todavía
db/          # migraciones sqlx, embebidas en el binario al compilar
docs/        # bóveda: contexto, plan, TODO, gotchas, decisiones
scripts/     # gates del chasis (vendored de chasis-kit; no se re-estilan acá)
```

Correr en local (necesita el Postgres de `docker-compose.yml`; `make db-up` lo levanta):

```bash
make db-up && DATABASE_URL=postgres://td:td@localhost:55432/task_dashboard cargo run -p task-dashboard-api
```

`curl localhost:8080/health`. Hacen falta también `SYNTROAUTH_ISSUER`, `SYNTROAUTH_AUDIENCE` y
`SYNTROAUTH_JWKS_URL` (ver `.env.example`); `GET /api/me` con un `Bearer` de syntroAuth devuelve
el `sub`. Con ese JWT, la API mínima: `POST /api/projects`, `POST /api/projects/{slug}/members`,
`POST /api/projects/{slug}/tokens` (devuelve el PAT `tdp_…` una sola vez),
`PUT /api/projects/{slug}/sequences/{prefix}` (seed). Detalle en `docs/PLAN-PASO-1.md`.
Las migraciones (`db/migrations`) corren solas al arrancar.

El MCP vive en `POST /mcp` (Streamable HTTP) y se autentica con el PAT. Alta en Claude Code:

```bash
claude mcp add --transport http task-dashboard https://<app>.up.railway.app/mcp --header "Authorization: Bearer tdp_..."
```

Herramientas: `reservar_id`, `tomar_ficha`, `liberar_ficha`, `fichas_tomadas`, `sugerir`. El
proyecto y el usuario salen del PAT; ninguna acepta `proyecto` ni `quien`. Los
tests de base crean una base efímera por test contra `TEST_DATABASE_URL` (la exporta el
`Makefile`); `make gate` corre todo.

## Deploy

Railway: proyecto `stellar-wisdom` → servicio `task-dashboard-api` + `Postgres`. URL:
`https://task-dashboard-api-production-9cb0.up.railway.app`. La infra se opera por el MCP de
Railway desde Claude Code (ids en `docs/TODO.md` / memoria del agente), no a mano.

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
