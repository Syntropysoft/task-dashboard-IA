# TODO — task-dashboard-IA

Fuente de verdad del estado. Lo que no está acá, no está pendiente.

> Las invariantes y el gate están en `.claude/rules/00-chasis.md`; las trampas verificadas en
> [GOTCHAS.md](GOTCHAS.md); las decisiones cerradas en [DECISIONS/](DECISIONS/README.md). El plan
> detallado del paso 1 está en [PLAN-PASO-1.md](PLAN-PASO-1.md). Acá va **solo lo accionable**.

---

## Paso 1 — backend + MCP-vivo (fecha dura: 2026-09-30)

Los pasos numerados y su verificación están en `docs/PLAN-PASO-1.md`. Acá solo el estado:

- [ ] 0. Stack decidido (Rust recomendado; alternativa .NET AOT con spike previo)
- [ ] 1. Scaffolding + `/health` + Dockerfile + deploy vacío en Railway + Postgres
- [ ] 2. Migraciones con el esquema
- [ ] 3. Auth bearer → dev
- [ ] 4. `reservar_id` + test de concurrencia
- [ ] 5. `tomar_ficha` / `liberar_ficha` / `fichas_tomadas`
- [ ] 6. `sugerir`
- [ ] 7. MCP sobre `/mcp` con las 5 herramientas, probado desde Claude Code
- [ ] 8. Seed del contador con el máximo ID real de Convertix
- [ ] 9. Alta en los dos Claude Code + regla en el `CLAUDE.md` de Convertix
- [ ] 10. Firestore `backlog-mnc` apagado

## Decisiones abiertas

- [ ] **¿Andrés lo va a usar?** Sin los dos, se repite lo de Firestore. Es el riesgo #1.
- [ ] **¿Firestore vence de verdad el 2026-09-30 o es extensible?** Cambia la urgencia de todo.
- [ ] **Lectura del repo (paso 2): ¿solo `main` o por rama?** Decide si `proxima_ficha()` puede
      ofrecer una ficha ya cerrada en una feature branch.
- [ ] **Contrato del frontmatter de Convertix** (`prioridad-iniciativa`, prioridad de ficha,
      "Depende de", `⛔ Trello`, `tema`): este proyecto lo consume, el gate de Convertix lo cuida.
      Hay que escribirlo antes del paso 2.

## Chasis (pendientes de la instalación)

- [x] `/td-plan` probada con input imperfecto ("paso 4"): normalizó al ítem 4 del paso 1, declaró
      la dependencia de los ítems 1–3 y marcó el modelo `SIN DECLARAR` (2026-09-13).
- [x] `docs-ratchet` visto dar ROJO con `docs/SUELTO.md` huérfano inyectado (2026-09-13).
- [ ] ⛔ Andrés: `make hooks` en su clon y confirmar que el pre-commit le corre.

## Pasos siguientes (no planificados todavía)

- Paso 2: indexador de solo lectura (webhook de push de GitHub) + `proxima_ficha()` + `estado(tema)`.
- Paso 3: frontend del dashboard.

---

## Hecho

- 2026-09-13 — Contexto inicial, decisión de infra (Railway, un servicio, Postgres 1 vCPU/1 GB),
  plan del paso 1, repo y chasis `/td-*` desde chasis-kit v14 (Claude Code). Gates
  `chasis-check`/`contexto-check` vistos en ROJO con fallas inyectadas antes del primer commit;
  `docs-linkcheck` forkeado para excluir `.claude/` (12 huérfanos falsos).
