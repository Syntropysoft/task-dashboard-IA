# Plan — Paso 1: backend + MCP-vivo (reemplazo de Firestore)

> Escrito el 2026-09-13, reescrito el mismo día tras dos decisiones: el producto sirve a
> **cualquier proyecto** (no solo Convertix) y la identidad la da **syntroAuth**. Contexto en
> `docs/CONTEXTO-INICIAL.md`. Fecha límite: **2026-09-30** (Firestore se extiende un mes como
> red, pero el objetivo no se mueve). Este paso NO incluye indexador, login web ni frontend.

## Objetivo

Que dos agentes de un mismo proyecto nunca reciban el mismo ID ni ejecuten la misma ficha, y que
las sugerencias sueltas tengan un lugar donde caer. Todo lo demás sigue en el repo del proyecto.

## Alcance

**Entra:** proyectos y membresía (por API, sin pantalla) · JWT de syntroAuth validado localmente ·
tokens de acceso (PAT) para el MCP · reserva atómica de IDs · claims · sugerencias · deploy en
Railway · alta del MCP en los dos Claude Code · Convertix como proyecto #1 · baja de Firestore.

**No entra:** leer el repo, `proxima_ficha()`, `estado(tema)`, frontend, indexador, pantalla de
proyectos/tokens. El esquema los contempla para no bloquearlos.

## Infra (Railway, un proyecto)

| Servicio | Qué | Config |
| :--- | :--- | :--- |
| `app` | Binario Rust en Docker, puerto `$PORT` | App sleeping ON · `DATABASE_URL` · `SYNTROAUTH_ISSUER` · `SYNTROAUTH_JWKS_URL` · `SYNTROAUTH_AUDIENCE` · `RUST_LOG` |
| `postgres` | Postgres gestionado | 1 vCPU / 1 GB · backups automáticos de Railway |

syntroAuth ya corre en Railway; no se toca. Dockerfile multi-stage: build con `rust:1-slim`,
runtime `distroless/cc`. Objetivo: imagen < 30 MB, RAM en reposo < 40 MB.

## Identidad y autorización (regla de la suite: syntroAuth autentica, la app autoriza)

- **Usuario** = `sub` del JWT de syntroAuth. No hay tabla de usuarios propia: solo membresías.
- **`/api/*`** acepta `Authorization: Bearer <JWT>` validado contra el JWKS
  (`iss`, `aud`, `exp`, firma). Antes de una operación destructiva (revocar token, borrar
  proyecto) se llama a `GET /api/auth/validate` de syntroAuth — contrato híbrido de
  `SSO_SUITE_GUIDE.md` §1.3.
- **`/mcp`** acepta `Authorization: Bearer tdp_<token>`: un **PAT** emitido por esta app,
  hasheado, revocable, **ligado a un usuario y a UN proyecto**. Un JWT de 30 min no sirve para
  un agente; el PAT es lo que reemplaza al "token por dev" del diseño anterior.
- El proyecto de cada llamada MCP sale del PAT. Ninguna herramienta acepta `proyecto` ni
  `quien` por parámetro.
- Toda query lleva `project_id` en el `WHERE`. Sin excepción, fail-closed: sin proyecto
  resuelto no hay query.

## Esquema

```sql
-- Un proyecto = un repo. Lo que hoy es Convertix/motor-ventas es la fila #1.
create table projects (
  id              uuid primary key default gen_random_uuid(),
  slug            text not null unique,        -- 'convertix'
  name            text not null,
  repo_url        text not null,               -- https://github.com/.../motor-ventas
  branch          text not null default 'main',-- la rama que lee el indexador (paso 2)
  frontmatter_map jsonb not null default '{}', -- mapeo de campos, ver docs/CONTRATO-FRONTMATTER.md
  created_by      text not null,               -- sub de syntroAuth
  created_at      timestamptz not null default now()
);

create table project_members (
  project_id  uuid not null references projects(id) on delete cascade,
  user_sub    text not null,
  role        text not null default 'member',  -- owner | member
  added_at    timestamptz not null default now(),
  primary key (project_id, user_sub)
);

-- PAT para el MCP. Un token = un usuario en un proyecto. Se muestra una sola vez al crearlo.
create table access_tokens (
  id            uuid primary key default gen_random_uuid(),
  project_id    uuid not null references projects(id) on delete cascade,
  user_sub      text not null,
  name          text not null,                 -- 'claude-code mac gabriel'
  token_hash    text not null unique,          -- sha256 del secreto; el prefijo tdp_ no se hashea
  created_at    timestamptz not null default now(),
  last_used_at  timestamptz,
  revoked_at    timestamptz
);

-- Contador por (proyecto, prefijo). La atomicidad es el UPDATE con row lock.
create table id_sequences (
  project_id  uuid not null references projects(id) on delete cascade,
  prefix      text not null,                   -- 'MVC'
  next        integer not null,
  primary key (project_id, prefix)
);

-- Auditoría de cada reserva (los huecos reservado≠usado son aceptables y quedan a la vista).
create table id_reservations (
  project_id  uuid not null references projects(id) on delete cascade,
  id          text not null,                   -- 'MVC-0407'
  prefix      text not null,
  number      integer not null,
  reserved_by text not null,                   -- user_sub
  reserved_at timestamptz not null default now(),
  primary key (project_id, id)
);

-- Quién tiene qué ficha AHORA. Una fila por ficha; sin historial.
create table claims (
  project_id  uuid not null references projects(id) on delete cascade,
  ficha_id    text not null,
  held_by     text not null,                   -- user_sub
  held_since  timestamptz not null default now(),
  note        text,
  primary key (project_id, ficha_id)
);

create table suggestions (
  id          bigserial primary key,
  project_id  uuid not null references projects(id) on delete cascade,
  author      text not null,                   -- user_sub
  text        text not null,
  context     text,
  status      text not null default 'open',    -- open | promoted | discarded
  ficha_id    text,
  created_at  timestamptz not null default now()
);
```

## API mínima (`/api`, JWT) — lo justo para operar sin pantalla

| Endpoint | Quién | Para qué |
| :--- | :--- | :--- |
| `POST /api/projects` | cualquier usuario autenticado | crea el proyecto; el creador queda `owner` |
| `POST /api/projects/{slug}/members` | owner | agrega un `user_sub` (Andrés se registra en syntroAuth y se lo agrega) |
| `POST /api/projects/{slug}/tokens` | member | emite un PAT propio; devuelve el secreto **una vez** |
| `DELETE /api/projects/{slug}/tokens/{id}` | dueño del token u owner | revoca (previo `/validate` en syntroAuth) |
| `PUT /api/projects/{slug}/sequences/{prefix}` | owner | seed del contador (`next`) — el paso 8 |

## Contrato de las herramientas MCP (`/mcp`, PAT)

Transporte: **Streamable HTTP**. Sin PAT válido → 401 antes de llegar al MCP. Proyecto y usuario
salen del PAT.

| Herramienta | Entrada | Salida | Errores explícitos |
| :--- | :--- | :--- | :--- |
| `reservar_id` | `prefijo` | `{ id: "MVC-0407" }` | `PREFIJO_DESCONOCIDO` (no hay seed para ese prefijo en este proyecto) |
| `tomar_ficha` | `ficha_id`, `nota?`, `force?` | `{ ok, held_by, held_since }` | `YA_TOMADA` (quién y desde cuándo; con `force` la pisa y queda en el log con ambos) |
| `liberar_ficha` | `ficha_id` | `{ ok }` | `NO_ES_TUYA` (salvo `force`), `NO_TOMADA` |
| `fichas_tomadas` | — | lista `{ ficha_id, held_by, held_since, note }` | — |
| `sugerir` | `texto`, `contexto?` | `{ id }` | — |

Reglas del contrato que van también al `CLAUDE.md` de cada proyecto consumidor:

1. **Si `reservar_id` falla o no responde, el agente NO inventa un ID.** Un reintento con espera
   es aceptable (el servicio puede estar despertando); inventar, nunca.
2. Antes de trabajar una ficha, `tomar_ficha`. Al cerrarla por commit, `liberar_ficha`.
3. `force` es para claims huérfanos. Quien fuerza asume el choque; queda en el log.

Sin TTL automático: con `fichas_tomadas` + `force` alcanza. Se agrega si molesta.

## Pasos, en orden, con verificación

| # | Qué | Cómo se verifica |
| :--- | :--- | :--- |
| 0 | Decidir Rust (o spike de un día de .NET AOT). | Decisión en `docs/CONTEXTO-INICIAL.md`. |
| 1 | `cargo init` en `apps/api`, `axum` con `/health`, Dockerfile, deploy vacío a Railway + Postgres. | `curl /health` → 200. Imagen y RAM medidas contra el objetivo. |
| 2 | Migraciones (`sqlx migrate`) con el esquema. Corren al arrancar. | Tablas existen; deploy idempotente. |
| 3a | Validación de JWT contra el JWKS de syntroAuth (`iss`/`aud`/`exp`/firma), cache del JWKS. | Test con un JWT real de syntroAuth (Development) → `GET /api/me` devuelve el `sub`; JWT vencido/otro `aud` → 401. |
| 3b | `POST /api/projects`, `members`, `tokens` (emisión + revocación), `sequences`. | Con curl y un JWT: crear `convertix`, agregar un member, emitir un PAT. |
| 3c | Middleware PAT en `/mcp`: hash → (project, user), `last_used_at`, revocado → 401. | Test: PAT válido / revocado / inexistente. |
| 4 | `reservar_id`. | 50 llamadas en paralelo con el mismo PAT → 50 números distintos y consecutivos; dos proyectos con el mismo prefijo no se pisan. |
| 5 | `tomar_ficha` / `liberar_ficha` / `fichas_tomadas`. | Tomar dos veces → `YA_TOMADA`; liberar ajena → `NO_ES_TUYA`; `force` funciona y loguea; **un PAT de otro proyecto no ve el claim**. |
| 6 | `sugerir`. | Inserta y devuelve id, con el `project_id` del PAT. |
| 7 | `rmcp` sobre `/mcp` con las 5 herramientas. | `claude mcp add --transport http …` con el PAT en el header; llamada real desde una sesión. |
| 8 | Seed de Convertix: `PUT …/sequences/MVC` con el máximo ID real del repo + 1 (abiertas, DONE y Trello). | Comparar contra `grep` en motor-ventas; anotar el número. |
| 9 | ⛔ Andrés: registro en syntroAuth, alta como member, su PAT, alta del MCP en su Claude Code; regla en el `CLAUDE.md` de Convertix. | Cada uno reserva un ID y toma/libera una ficha de prueba. |
| 10 | Apagar Firestore `backlog-mnc`. | `grep backlog-mnc` en Convertix vacío. |

3a/3b/3c van juntos; 4, 5 y 6 son independientes una vez hecho el 3.

## Fail-paths que los tests tienen que cubrir

- Dos `reservar_id` simultáneos, mismo proyecto y prefijo → números distintos.
- Mismo prefijo en dos proyectos → contadores independientes.
- `reservar_id` con la base caída → error claro, nunca un número inventado ni repetido.
- PAT de un proyecto usado contra fichas/IDs de otro → como si no existieran (fail-closed).
- PAT revocado o JWT con `aud` ajeno → 401, sin distinguir "no existe" de "revocado".
- `tomar_ficha` de una ficha ya tomada por otro → `YA_TOMADA` con datos, no un `ok` silencioso.
- Reinicio del servicio a mitad de una reserva → la transacción se revierte.
- JWKS de syntroAuth inaccesible al arrancar → el servicio arranca, `/mcp` (PAT) sigue
  funcionando, `/api` (JWT) responde 503 hasta poder validar — nunca acepta sin firma.
- Servicio dormido → primera llamada tarda unos segundos; timeout del cliente MCP ≥ 15 s.

## Decisiones que este plan deja tomadas

- syntroAuth autentica; esta app autoriza (proyectos, membresía, PAT). Sin usuarios propios.
- Un PAT = un usuario en un proyecto. El proyecto nunca viaja por parámetro.
- `project_id` en toda tabla y toda query desde la primera migración.
- Sin TTL de claims; sin historial de claims; huecos en la numeración aceptables.
- Postgres desde el día uno.

## Abierto (fuera de este paso, condiciona el 2)

- `docs/CONTRATO-FRONTMATTER.md`: el schema de `projects.frontmatter_map` y lo que el indexador
  exige del repo (`prioridad-iniciativa`, prioridad de ficha, "Depende de", `⛔`, `tema`).
- Cómo un proyecto autoriza al indexador a leer su repo (token de GitHub por proyecto, o GitHub App).
