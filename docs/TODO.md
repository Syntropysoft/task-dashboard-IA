# TODO — task-dashboard-IA

Fuente de verdad del estado. Lo que no está acá, no está pendiente.

> Las invariantes y el gate están en `.claude/rules/00-chasis.md`; las trampas verificadas en
> [GOTCHAS.md](GOTCHAS.md); las decisiones cerradas en [DECISIONS/](DECISIONS/README.md). El plan
> detallado del paso 1 está en [PLAN-PASO-1.md](PLAN-PASO-1.md). Acá va **solo lo accionable**.

---

## Paso 1 — backend + MCP-vivo (fecha dura: 2026-09-30)

Los pasos numerados y su verificación están en `docs/PLAN-PASO-1.md`. Acá solo el estado:

- [ ] 0. Stack decidido (Rust recomendado; alternativa .NET AOT con spike previo)
- [x] 1. Scaffolding + `/health` + Dockerfile — medido 2026-09-13: imagen 8 MB, RAM ~1 MB,
      `/health` 200 en 3 ms, SIGTERM limpio. CI en `.github/workflows/ci.yml`.
      - [ ] ⛔ Gabriel: proyecto en Railway con el repo conectado (deploy automático desde
        `main`, builder Dockerfile) + servicio Postgres 1 vCPU/1 GB + app sleeping. Verificar
        `curl https://<app>.up.railway.app/health`.
- [x] 2. Migraciones con el esquema (`db/migrations/20260913000001_init.sql`, embebidas, corren al
      arrancar; idempotencia y `project_id NOT NULL` en toda tabla verificados por test contra
      Postgres efímero). Sin `DATABASE_URL` el servicio no arranca (2026-09-13).
- [x] 3a. JWT de syntroAuth validado contra JWKS: RS256, `iss`/`aud`/`exp`/`sub` obligatorios,
      cache en memoria con un refresh por kid desconocido cada 30 s, 401 opaco, 503 si el JWKS
      nunca cargó. `GET /api/me`. 9 tests con un syntroAuth falso (2026-09-13).
      - [ ] ⛔ Gabriel: confirmar que el syntroAuth de Railway firma **RS256** (`Jwt:PrivateKeyPath`
        + `PublicKeyPath`; `GET /.well-known/jwks.json` no da 404) y pasar la URL pública y el
        `iss`/`aud` reales para `SYNTROAUTH_*` en Railway. Con HS256 esta app no valida nada.
- [ ] 3b. API mínima: projects / members / tokens (PAT) / sequences
- [ ] 3c. Middleware PAT en `/mcp`
- [ ] 4. `reservar_id` + test de concurrencia
- [ ] 5. `tomar_ficha` / `liberar_ficha` / `fichas_tomadas`
- [ ] 6. `sugerir`
- [ ] 7. MCP sobre `/mcp` con las 5 herramientas, probado desde Claude Code
- [ ] 8. Seed del contador `MVC` de Convertix con el máximo ID real del repo
- [ ] 9. ⛔ Andrés: registro en syntroAuth, member, PAT, alta del MCP + regla en el `CLAUDE.md` de Convertix
- [ ] 10. Firestore `backlog-mnc` apagado

## Decisiones abiertas

- [ ] ⛔ **Andrés** todavía no conoce la idea. Decidido 2026-09-13: se le muestra con el MCP
      andando (ítem 7), no antes. Hasta entonces, el ítem 9 está bloqueado por él.
- [x] **Firestore es extensible.** Decidido 2026-09-13: se extienden las reglas de `backlog-mnc`
      un mes como red; el objetivo sigue siendo el 2026-09-30. — [ ] ⛔ hacer la extensión en la
      consola de Firebase (acción de Gabriel, fuera de este repo).
- [x] **Lectura del repo (paso 2)**: configuración por proyecto (`projects.branch`, default
      `main`). Regla: lo que no está en esa rama no existe para el dashboard; el claim vivo cubre
      el trabajo en otras ramas (2026-09-13).
- [ ] **Contrato del frontmatter** (`prioridad-iniciativa`, prioridad de ficha, "Depende de",
      `⛔`, `tema`). Decidido 2026-09-13: se escribe **acá**, en `docs/CONTRATO-FRONTMATTER.md`,
      como schema de `projects.frontmatter_map` y spec del parser del indexador. Cada proyecto
      consumidor lo referencia. Antes del paso 2.
- [ ] **Acceso del indexador al repo de cada proyecto** (token de GitHub por proyecto vs GitHub
      App). Paso 2.
- [x] **Referencia de syntroAuth: la rama `develop`** — ahí está todo integrado (2026-09-13).
      Para leer su contrato, `git switch develop` en el clon de `source-2/syntropysoft/syntroAuth`.

## Decisiones cerradas el 2026-09-13 (segunda ronda)

- **Producto para cualquier proyecto**, no solo Convertix. Un proyecto = un repo (URL, rama,
  prefijos de ID, mapeo de frontmatter). Convertix es el proyecto #1.
- **Identidad: syntroAuth** (IdP de la suite, JWT RS256 + JWKS). Esta app autoriza: membresía y
  PATs. Sin usuarios propios ni proveedores OAuth propios.
- **Prioridad:** el paso 1 sigue primero, con `projects` y JWT desde el día uno. Login web y
  pantalla de proyectos después (paso 1.5, antes del indexador).

- **Redis: no** (2026-09-13). Hay uno en el proyecto de Railway (de syntroAuth), pero este servicio
  no lo usa: reserva/claims/PAT viven en Postgres con transacciones; el JWKS se cachea en memoria
  (un solo proceso). Se reabre solo si aparecen réplicas (rate limit o cache de revocación
  compartida). Compartir el de syntroAuth mezclaría estado de dos servicios.

## Chasis (pendientes de la instalación)

- [x] `/td-plan` probada con input imperfecto ("paso 4"): normalizó al ítem 4 del paso 1, declaró
      la dependencia de los ítems 1–3 y marcó el modelo `SIN DECLARAR` (2026-09-13).
- [x] `docs-ratchet` visto dar ROJO con `docs/SUELTO.md` huérfano inyectado (2026-09-13).
- [ ] ⛔ Andrés: `make hooks` en su clon y confirmar que el pre-commit le corre.

## Flujo de ramas de este repo (decidido 2026-09-13)

`develop` para trabajar y probar en local; `main` es lo desplegado: el merge a `main` deploya en
Railway. El pre-commit bloquea commits directos sobre `main` mientras exista `develop`.

## Pasos siguientes (no planificados todavía)

- Paso 2: indexador de solo lectura (webhook de push de GitHub) + `proxima_ficha()` + `estado(tema)`.
- Paso 3: frontend del dashboard.

---

## Hecho

- 2026-09-13 — Contexto inicial, decisión de infra (Railway, un servicio, Postgres 1 vCPU/1 GB),
  plan del paso 1, repo y chasis `/td-*` desde chasis-kit v14 (Claude Code). Gates
  `chasis-check`/`contexto-check` vistos en ROJO con fallas inyectadas antes del primer commit;
  `docs-linkcheck` forkeado para excluir `.claude/` (12 huérfanos falsos).
