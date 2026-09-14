-- Esquema inicial. Multi-proyecto desde el día uno: project_id en toda tabla de datos.
-- ❌ NEVER editar esta migración después de aplicada: sqlx verifica el checksum al arrancar y
-- el servicio se niega a levantar. Lo nuevo va en una migración nueva.
-- Referencia: docs/PLAN-PASO-1.md § Esquema.

create extension if not exists pgcrypto; -- gen_random_uuid()

-- Un proyecto = un repo. Convertix/motor-ventas es la fila #1.
create table projects (
  id              uuid primary key default gen_random_uuid(),
  slug            text not null unique,
  name            text not null,
  repo_url        text not null,
  branch          text not null default 'main',
  frontmatter_map jsonb not null default '{}'::jsonb,
  created_by      text not null,                 -- sub de syntroAuth
  created_at      timestamptz not null default now(),
  constraint projects_slug_format check (slug ~ '^[a-z0-9][a-z0-9-]{1,62}$')
);

create table project_members (
  project_id  uuid not null references projects(id) on delete cascade,
  user_sub    text not null,
  role        text not null default 'member',
  added_at    timestamptz not null default now(),
  primary key (project_id, user_sub),
  constraint project_members_role check (role in ('owner', 'member'))
);

-- PAT para el MCP: un usuario en un proyecto. Solo el hash; el secreto se muestra una vez.
create table access_tokens (
  id            uuid primary key default gen_random_uuid(),
  project_id    uuid not null references projects(id) on delete cascade,
  user_sub      text not null,
  name          text not null,
  token_hash    text not null unique,
  created_at    timestamptz not null default now(),
  last_used_at  timestamptz,
  revoked_at    timestamptz
);
create index access_tokens_project_user on access_tokens (project_id, user_sub);

-- Contador por (proyecto, prefijo). La atomicidad es el UPDATE con row lock.
create table id_sequences (
  project_id  uuid not null references projects(id) on delete cascade,
  prefix      text not null,
  next        integer not null,
  primary key (project_id, prefix),
  constraint id_sequences_next_positive check (next > 0)
);

-- Auditoría de cada reserva. Huecos (reservado ≠ usado) son aceptables y quedan a la vista.
create table id_reservations (
  project_id  uuid not null references projects(id) on delete cascade,
  id          text not null,
  prefix      text not null,
  number      integer not null,
  reserved_by text not null,
  reserved_at timestamptz not null default now(),
  primary key (project_id, id),
  unique (project_id, prefix, number)
);

-- Quién tiene qué ficha AHORA. Una fila por ficha; sin historial.
create table claims (
  project_id  uuid not null references projects(id) on delete cascade,
  ficha_id    text not null,
  held_by     text not null,
  held_since  timestamptz not null default now(),
  note        text,
  primary key (project_id, ficha_id)
);

create table suggestions (
  id          bigserial primary key,
  project_id  uuid not null references projects(id) on delete cascade,
  author      text not null,
  text        text not null,
  context     text,
  status      text not null default 'open',
  ficha_id    text,
  created_at  timestamptz not null default now(),
  constraint suggestions_status check (status in ('open', 'promoted', 'discarded'))
);
create index suggestions_project_status on suggestions (project_id, status);
