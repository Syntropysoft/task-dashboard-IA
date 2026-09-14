# task-dashboard-IA

Coordinación entre dos devs y sus agentes sobre el mismo repo: reserva atómica de IDs, claims de
fichas (quién tiene qué AHORA) y sugerencias sueltas — lo que el markdown del repo no puede
representar. El estado de las fichas sigue viviendo en el repo del proyecto; esto lo **lee**, no lo
copia. **Un dato, un dueño.**

## Estado

Sin código todavía. Está el contexto, el plan del paso 1 y el chasis de trabajo.

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
**Postgres** gestionado (1 vCPU / 1 GB). Lenguaje: Rust. Fecha dura del paso 1: **2026-09-30**.

Layout previsto del monorepo — se crea cuando entra el código, no antes:

```
apps/api/    # Rust: axum + rmcp + sqlx. Sirve /mcp, /api, /health y los estáticos de web.
apps/web/    # frontend del dashboard (paso 3)
db/          # migraciones sqlx
docs/        # bóveda: contexto, plan, TODO, gotchas, decisiones
scripts/     # gates del chasis (vendored de chasis-kit; no se re-estilan acá)
```

## Cómo se trabaja

Al clonar, **una vez**:

```bash
make hooks
```

Cablea `.githooks/pre-commit` en este clon (`core.hooksPath` es local y no viaja). Sin eso, el
commit no corre ninguna red. El gate completo es `make gate`; mientras no exista `Cargo.toml` corre
solo los gates de Node y lo dice.
