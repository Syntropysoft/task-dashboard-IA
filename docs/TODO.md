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
      - [x] Railway armado por MCP el 2026-09-13: proyecto `stellar-wisdom`, servicio
        `task-dashboard-api` conectado a `main` (Dockerfile, healthcheck `/health`, sleeping ON,
        ON_FAILURE×3), Postgres (template oficial, volumen 5 GB) y
        `https://task-dashboard-api-production-9cb0.up.railway.app`. Variables: `DATABASE_URL`
        referencia al Postgres; `SYNTROAUTH_*` con `JWKS_URL` **placeholder**.
      - [x] PR #1 y #2 mergeados por Gabriel (2026-09-14). Verificado en producción: `/health`
        200 (1,4 s despertando), log `base conectada y migraciones al día`, app sleeping
        funcionando (duerme a los ~8 min, despierta con la primera request).
      - [x] `SYNTROAUTH_JWKS_URL` real cargada (2026-09-14). syntroAuth vive en otra cuenta de Railway.
- [x] 2. Migraciones con el esquema (`db/migrations/20260913000001_init.sql`, embebidas, corren al
      arrancar; idempotencia y `project_id NOT NULL` en toda tabla verificados por test contra
      Postgres efímero). Sin `DATABASE_URL` el servicio no arranca (2026-09-13).
- [x] 3a. JWT de syntroAuth validado contra JWKS: RS256, `iss`/`aud`/`exp`/`sub` obligatorios,
      cache en memoria con un refresh por kid desconocido cada 30 s, 401 opaco, 503 si el JWKS
      nunca cargó. `GET /api/me`. 9 tests con un syntroAuth falso (2026-09-13).
      - [x] syntroAuth de producción: `https://syntroauth-production.up.railway.app`, firma RS256
        (`kid SyntroAuth-1`), `iss`/`aud` = `SyntroAuth`. Variables cargadas en Railway el
        2026-09-14; `/api/me` con token basura → 401 (JWKS cargado).
- [x] 3b. API mínima con JWT: `GET/POST /api/projects`, `POST …/members` (owner), `GET/POST
      …/tokens` (PAT `tdp_…`, secreto una vez, sha256 en base), `DELETE …/tokens/{id}` (propio u
      owner; exige `/api/auth/validate` de syntroAuth, fail-closed), `PUT …/sequences/{prefix}`
      (owner; **nunca baja** el contador → 409). No miembro = 404. 6 tests contra Postgres real
      (2026-09-13).
- [x] 3c. Middleware PAT en `/mcp` (`auth/pat.rs`): `Bearer tdp_…` → sha256 → (proyecto, usuario)
      en las extensions; inexistente / revocado / prefijo ajeno / JWT → el mismo 401;
      `last_used_at` como mucho una vez por minuto. Sonda `GET /mcp/whoami` hasta que llegue
      rmcp (ítem 7). 4 tests contra Postgres real, con PATs emitidos por la API (2026-09-14).
- [x] 4. `reservar_id` (`ids.rs` + `POST /mcp/reservar_id` con PAT): `UPDATE … RETURNING` sobre el
      contador + auditoría en la misma transacción. Test: 50 tareas en paralelo → 50 números
      distintos y consecutivos, contador y auditoría exactos; dos proyectos con el mismo prefijo
      independientes; sin seed → `PREFIJO_DESCONOCIDO` (404) y sin rastro (2026-09-14).
- [x] 5. `tomar_ficha` / `liberar_ficha` / `fichas_tomadas` (`claims.rs` + rutas `/mcp/*` con PAT):
      transacción con `FOR UPDATE`; `YA_TOMADA` (409) con quién y desde cuándo; retomar la propia
      es idempotente; `force` pisa, loguea ambos y devuelve `previous_holder`; `NO_ES_TUYA` (403),
      `NO_TOMADA` (409). Test: 20 tomas simultáneas de una ficha nueva → exactamente una gana;
      aislamiento por proyecto (2026-09-14).
- [x] 6. `sugerir` (`suggestions.rs` + `POST /mcp/sugerir`, `GET /mcp/sugerencias` con PAT): inserta
      con el proyecto y el autor del PAT, devuelve `{id}`; lista las abiertas del proyecto, más
      viejas primero; texto vacío o > 4000 chars (contexto > 500) → 400 sin rastro (2026-09-14).
- [x] 7. MCP real (rmcp 3.3, Streamable HTTP) en `POST /mcp` detrás del PAT, con las 5 herramientas
      y las reglas del contrato en `instructions`. Sesiones en memoria (`LocalSessionManager`):
      el modo sin sesión de rmcp solo sirve a clientes del protocolo 2026-07-28, y Claude Code
      hace handshake. Errores de dominio como `is_error` con `CODIGO: explicación`. 5 tests
      end-to-end por socket real: handshake, tools/list, flujo completo, errores, 401, host
      ajeno, sesión perdida → 404 (2026-09-14).
      - [x] Desplegado en Railway (PR #3, 2026-09-14): `POST /mcp` sin PAT → 401 verificado.
      - [ ] ⛔ Probar desde Claude Code real contra Railway (ítem 9): `claude mcp add --transport
        http task-dashboard <url>/mcp --header "Authorization: Bearer tdp_…"`. Necesita un PAT,
        que necesita `/api`, que necesita la URL de syntroAuth.
- [x] 8. Seed de Convertix — **hecho por SQL el 2026-09-14** (bootstrap idempotente contra el
      Postgres de Railway, decisión de Gabriel porque el login de syntroAuth está roto): proyecto
      `convertix` (motor-ventas, `main`), Gabriel owner (`sub 0bc75872-…`), `MVC → 407` y
      `FE → 125` (máximos medidos en `motor-ventas@develop e8667f5`: MVC-0406, FE-0124). Segunda
      corrida: sin filas nuevas (los upserts reafirman las mismas 2 filas), contadores sin cambio. Si pasan días antes de usarlo, re-medir y subir por
      `PUT /api/projects/convertix/sequences/{prefijo}` (solo sube).
- [ ] 9. ⛔ Andrés: registro en syntroAuth, member, PAT, alta del MCP + regla en el `CLAUDE.md` de Convertix
- [ ] 10. Firestore `backlog-mnc` apagado

## Decisiones abiertas

- [ ] ⛔ **syntroAuth (repo aparte)**: su base de producción (Postgres compartido con n8n) tiene
      el schema **`janus`** (tablas `janus.users`, `janus.tenants`…), pero el código desplegado
      consulta **`syntro_auth`** → `42P01: relation "syntro_auth.tenants" does not exist`. El
      código quedó adelante de la base (rename de schema sin migrar). Hasta que se arregle, el
      login de syntroAuth en producción no funciona y el ítem 9 está bloqueado. Además el login
      exige la password cifrada con `/api/auth/security/public-key` (como hace su frontend).
      Medido 2026-09-14 con acceso de solo lectura a esa base.
- Datos para el bootstrap (2026-09-14): `sub` de Gabriel = `0bc75872-d050-42f0-8529-a656209da8c2`
  (id en `janus.users`); tenant de la suite = `a0000000-0000-0000-0000-000000000001` ("Default").

- [ ] `hipótesis`: tras un app sleeping / redeploy la sesión MCP en memoria se pierde, el servidor
      responde 404 y **el cliente de Claude Code re-inicializa solo** (la spec MCP lo exige al
      cliente). Validar en el ítem 9 con Claude Code real; si no lo hace, alternativa: sesiones
      en Postgres (`session_store` de rmcp) o apagar el sleeping. `TODO: validar` en
      `apps/api/src/mcp.rs`.
- [x] **Formato del PAT: `tdp_<key_id>.<secret>`** — decidido e implementado 2026-09-14: key_id =
      id de la fila (en claro: buscar, loguear, revocar), secret hasheado y comparado en tiempo
      constante. Migración `20260914000001_pat_key_id.sql` (renombra `token_hash` → `secret_hash`).

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

## Operación

- [ ] ⛔ Gabriel: **cerrar el proxy TCP público de Postgres** (`altaria.proxy.rlwy.net:27317`)
      cuando todo esté desplegado y no haga falta entrar desde local. Decidido 2026-09-13: la
      pública es solo para desarrollo/pruebas.
- [x] Los tests de auth/api generaban una clave RSA por test (~16 s por binario). Ahora hay un
      depósito por binario (`KEY_STORE`, `OnceLock`) que se crea con la primera clave pedida y
      muere con el proceso (2026-09-14).

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
- Paso 3: frontend del dashboard. Maqueta hecha en Claude Design el 2026-09-14 (estáticas):
  https://claude.ai/code/artifact/87476512-c5d9-45bc-bbd5-15b63a6911d3 — dirección C "Tablero"
  elegida entre tres; paleta triádica del círculo cromático desde el naranja (oklch, matices
  45°/165°/285°: naranja = acción, verde = vivo, violeta = sugerencias) y marca = triángulo en
  los tres colores. Pantallas: Login (syntroAuth), Proyectos, Proyecto · En vivo / Tokens /
  Contadores / Miembros. El código de `apps/web` va contra esa maqueta.

---

## Hecho

- 2026-09-13 — Contexto inicial, decisión de infra (Railway, un servicio, Postgres 1 vCPU/1 GB),
  plan del paso 1, repo y chasis `/td-*` desde chasis-kit v14 (Claude Code). Gates
  `chasis-check`/`contexto-check` vistos en ROJO con fallas inyectadas antes del primer commit;
  `docs-linkcheck` forkeado para excluir `.claude/` (12 huérfanos falsos).
