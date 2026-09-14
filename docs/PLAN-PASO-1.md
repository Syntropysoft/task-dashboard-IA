# Plan — Paso 1: backend + MCP-vivo (reemplazo de Firestore)

> Escrito el 2026-09-13. Contexto en `docs/CONTEXTO-INICIAL.md`. Fecha límite dura: **2026-09-30**
> (vencen las reglas abiertas de Firestore `backlog-mnc`). Este paso NO incluye el indexador
> ni el frontend: solo lo que hoy hace a medias Firestore, hecho bien.

## Objetivo

Que dos agentes (uno por dev) nunca reciban el mismo ID ni ejecuten la misma ficha, y que las
sugerencias sueltas tengan un lugar donde caer. Todo lo demás sigue en el repo de Convertix.

## Alcance

**Entra:** reserva atómica de IDs · claims (tomar/liberar/listar) · sugerencias · auth por
token · deploy en Railway · alta del MCP en los dos Claude Code · baja de Firestore.

**No entra:** leer el repo, `proxima_ficha()`, `estado(tema)`, frontend, indexador. Esos
dependen del paso 2 y se listan acá solo para que el esquema no los bloquee.

## Infra (Railway, un proyecto)

| Servicio | Qué | Config |
| :--- | :--- | :--- |
| `app` | Binario Rust en Docker, puerto `$PORT` | App sleeping ON · `DATABASE_URL`, `DEV_TOKENS`, `RUST_LOG` |
| `postgres` | Postgres gestionado | 1 vCPU / 1 GB · backups automáticos de Railway |

Dockerfile multi-stage: build con `rust:1-slim`, runtime `gcr.io/distroless/cc` (o `scratch`
con target musl). Objetivo: imagen < 30 MB, RAM en reposo < 40 MB.

## Esquema

```sql
-- Quién puede hablar con el servicio. La identidad sale del token, nunca de un parámetro.
create table devs (
  id          text primary key,          -- 'gabriel' | 'andres'
  token_hash  text not null unique,      -- sha256 del bearer token
  created_at  timestamptz not null default now()
);

-- Contador por prefijo. La atomicidad es el UPDATE con row lock: dos sesiones nunca
-- reciben el mismo número.
create table id_sequences (
  prefix  text primary key,              -- 'MVC'
  next    integer not null
);

-- Auditoría de cada reserva (y detecta huecos: reservado ≠ usado, y está bien que pase).
create table id_reservations (
  id          text primary key,          -- 'MVC-0407'
  prefix      text not null references id_sequences(prefix),
  number      integer not null,
  reserved_by text not null references devs(id),
  reserved_at timestamptz not null default now()
);

-- Quién tiene qué ficha AHORA. Una fila por ficha; no hay historial (no hace falta).
create table claims (
  ficha_id   text primary key,           -- 'MVC-0385'
  held_by    text not null references devs(id),
  held_since timestamptz not null default now(),
  note       text                        -- opcional: "en rama feat/x"
);

-- Hallazgos que todavía no son ficha.
create table suggestions (
  id         bigserial primary key,
  author     text not null references devs(id),
  text       text not null,
  context    text,                       -- ficha/archivo/tema desde donde surgió
  status     text not null default 'open', -- open | promoted | discarded
  ficha_id   text,                       -- si se promovió a ficha
  created_at timestamptz not null default now()
);
```

## Contrato de las herramientas MCP

Transporte: **Streamable HTTP** en `/mcp`. Auth: `Authorization: Bearer <token>`; sin token o
token inválido → 401 antes de llegar al MCP. El `dev` de cada llamada se resuelve del token.

| Herramienta | Entrada | Salida | Errores explícitos |
| :--- | :--- | :--- | :--- |
| `reservar_id` | `prefijo` (ej. `MVC`) | `{ id: "MVC-0407" }` | `PREFIJO_DESCONOCIDO` |
| `tomar_ficha` | `ficha_id`, `nota?`, `force?` | `{ ok, held_by, held_since }` | `YA_TOMADA` (incluye quién y desde cuándo; con `force: true` la pisa y lo registra en el log) |
| `liberar_ficha` | `ficha_id` | `{ ok }` | `NO_ES_TUYA` (salvo `force`), `NO_TOMADA` |
| `fichas_tomadas` | — | lista de `{ ficha_id, held_by, held_since, note }` | — |
| `sugerir` | `texto`, `contexto?` | `{ id }` | — |

Reglas del contrato que van también al `CLAUDE.md` de Convertix:

1. **Si `reservar_id` falla o no responde, el agente NO inventa un ID.** Avisa y espera. Es el
   bug original; el servicio puede estar despertando (app sleeping, unos segundos) — reintentar
   una vez con espera es aceptable, inventar no.
2. Antes de trabajar una ficha, `tomar_ficha`. Al cerrarla por commit, `liberar_ficha`.
3. `force` es para claims huérfanos (sesión muerta). Quien fuerza asume el choque; queda en el log
   con ambos nombres.

Sin TTL automático en este paso: con dos personas, ver `fichas_tomadas` y forzar es más simple
y no toma decisiones por nadie. Si molesta, se agrega después.

## Pasos, en orden, con verificación

| # | Qué | Cómo se verifica |
| :--- | :--- | :--- |
| 0 | Decidir Rust (o spike de un día de .NET AOT: SDK MCP + Npgsql + `PublishAot`). | Decisión anotada en `docs/CONTEXTO-INICIAL.md`. |
| 1 | Repo `task-dashboard-IA`: `cargo init`, `axum` con `/health`, Dockerfile, deploy vacío a Railway + servicio Postgres. | `curl https://<app>.railway.app/health` → 200. Imagen y RAM medidas contra el objetivo. |
| 2 | Migraciones (`sqlx migrate`) con el esquema de arriba. Corren al arrancar. | Tablas existen; el deploy es idempotente. |
| 3 | Auth: middleware bearer → `dev`. Tokens generados a mano, hasheados en `devs`. | Sin token → 401. Token válido → `/whoami` devuelve el dev. |
| 4 | `reservar_id`. | Test de concurrencia: 50 llamadas en paralelo → 50 números distintos y consecutivos. |
| 5 | `tomar_ficha` / `liberar_ficha` / `fichas_tomadas`. | Tests: tomar dos veces → `YA_TOMADA`; liberar ajena → `NO_ES_TUYA`; `force` funciona y loguea. |
| 6 | `sugerir`. | Inserta y devuelve id. |
| 7 | Montar `rmcp` sobre `/mcp` exponiendo las 5 herramientas. | Alta en Claude Code (`claude mcp add --transport http ...`) y llamada real desde una sesión. |
| 8 | **Seed**: `id_sequences.MVC.next` = máximo ID del repo de Convertix + 1 (contando fichas abiertas, DONE y Trello). | Comparar contra `grep` en el repo; anotar el número en el commit del seed. |
| 9 | Alta del MCP en el Claude Code de los dos devs + regla en `CLAUDE.md` de Convertix. | Cada uno reserva un ID y toma/libera una ficha de prueba. |
| 10 | Apagar Firestore `backlog-mnc` (antes del 2026-09-30). | Nadie lo referencia en Convertix (`grep backlog-mnc`). |

Los pasos 4, 5 y 6 son independientes entre sí una vez hecho el 3.

## Fail-paths que los tests tienen que cubrir

- Dos `reservar_id` simultáneos con el mismo prefijo → números distintos (paso 4).
- `reservar_id` con la base caída → error claro, nunca un número inventado ni repetido.
- `tomar_ficha` de una ficha ya tomada por el otro → `YA_TOMADA` con datos, no un `ok` silencioso.
- Token de un dev usado para liberar la ficha del otro sin `force` → `NO_ES_TUYA`.
- Reinicio del servicio a mitad de una reserva → la transacción se revierte; el siguiente número no salta más de uno (o salta, y está documentado como aceptable).
- Servicio dormido → primera llamada tarda unos segundos pero responde; el cliente MCP no la da por muerta (timeout ≥ 15 s).

## Decisiones que este plan deja tomadas

- La identidad viene del token; ninguna herramienta acepta `quien`.
- Sin historial de claims ni TTL: `fichas_tomadas` + `force` alcanzan para dos personas.
- Los huecos en la numeración (ID reservado y no usado) son aceptables y quedan auditados.
- Postgres desde el día uno, aunque SQLite alcanzaría: no migrar después vale más que los pocos
  dólares de diferencia.

## Abierto (fuera de este paso, pero condiciona el 2)

- ¿El indexador lee solo `main` o por rama? Decide si `proxima_ficha()` puede ofrecer una ficha
  que ya está cerrada en una feature branch.
- Contrato del frontmatter de Convertix (`prioridad-iniciativa`, prioridad de ficha, "Depende de",
  `⛔ Trello`, `tema`): este proyecto lo consume, el gate de Convertix lo cuida. Hay que escribirlo.
