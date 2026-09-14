# Chasis de tooling `task-dashboard-IA` (capa siempre-cargada)

> Esta es la ÚNICA pieza del chasis que se carga en toda sesión — por eso es corta y todo lo que
> solo aplica a una clase de tarea vive en archivo propio con un puntero. NO agregar nada que no
> cambie una decisión del agente. Instalado desde chasis-kit v14 el 2026-09-13.

---

## 0. Cómo se trabaja acá (aplica antes que cualquier otra cosa)

- **La frontera es tuya, el código es mío.** Qué hace y qué NO hace el servicio, qué promete el
  contrato de las herramientas MCP, qué se borra, qué vive en la base y qué en el repo de
  Convertix — son decisiones del usuario y **se preguntan**. Implementación, nombres, estructura,
  tests y verificación son del agente: se hacen, no se consultan.
- **El tell.** Si la respuesta a *"¿por qué así?"* es un **principio** ("no adivinamos IDs"),
  estás moviendo un límite → preguntá. Si es una **técnica** ("uso `UPDATE … RETURNING` porque
  el row lock lo hace atómico"), decidí y seguí.
- ❌ NEVER afirmar una negación ("no existe X") sin decir **dónde** se buscó.
- ❌ NEVER una sonda manual como evidencia: o se vuelve test, o se dice "verificado a mano, sin test".

## 1–2. Anatomía y formato de skills

→ **`.claude/authoring-skills.md`**. Leerlo antes de crear o editar una skill o un command.
No hace falta para ninguna otra tarea.

## 3. Los 4 bucles de valor

### 3.1 `verification-loop` — nada se cierra sin verificar
Toda tarea termina ejecutando el **gate real**, no una simulación: `make gate`, que encadena
`cargo clippy --all-targets -- -D warnings` → `cargo test` → `node scripts/chasis-check.mjs` →
`node scripts/contexto-check.mjs` → `node scripts/docs-linkcheck.mjs`.
⚠️ **Todavía no hay `Cargo.toml`**: hasta el paso 1 del plan, `make gate` corre solo los tres
gates de Node y lo dice. ❌ NEVER citar clippy/test como corridos mientras no exista el crate.
❌ NEVER dar por hecha una tarea porque "compila". El gate es la prueba, no el criterio propio.
✅ ALWAYS si el gate no corrió (falta infra, base caída), decirlo explícito — no fingir verde.

### 3.2 `eval-harness` — el fail-path es el entregable
Por cada camino feliz nuevo, enumerar y probar sus modos de falla. Los de este repo, como piso:
dos llamadas concurrentes al mismo recurso · base caída a mitad de una transacción · PAT
revocado o de otro proyecto · JWT vencido, con otro `aud` o con JWKS inaccesible · servicio
dormido (app sleeping) que tarda en responder · claim de una ficha que ya cerró en el repo. Happy-path solo no cuenta como cobertura.

### 3.3 `strategic-compact` — comprimir sin perder el hilo
En tareas largas, consolidar el estado en un punto de control legible ANTES de que el contexto se
sature. La fuente de verdad del estado es `docs/TODO.md`, no la memoria de la sesión (regla
anti-drift). Compactar es seguro exactamente cuando nada de valor vive solo en la conversación.

### 3.4 `continuous-learning` — lo aprendido se persiste
Un gotcha no trivial no muere con la sesión. El motor es **`/td-learn`**: clasifica el hallazgo y
lo persiste en el nivel correcto — ficha accionable en `docs/TODO.md` / memoria local / gotcha en
`docs/GOTCHAS.md` / ADR en `docs/DECISIONS/` — con la regla de maduración: NO todo se guarda, y la
barrera para un ADR es extremadamente alta.

## 4. Invariantes de `task-dashboard-IA` que TODA skill debe respetar

- ❌ NEVER copiar a la base o a un doc propio el estado que vive en el repo de Convertix (fichas,
  prioridades, pendientes, auditorías). ✅ ALWAYS leerlo del repo en el momento — **un dato, un
  dueño**: `PLAN-EJECUCION.md` copiaba el orden del backlog y se desincronizó 4 veces medidas
  hasta que hubo que retirarlo (2026-09).
- ❌ NEVER inventar, adivinar o "completar" un ID cuando `reservar_id` falla o no responde.
  ✅ ALWAYS error explícito y esperar (un reintento con espera es aceptable; inventar, nunca) —
  el 2026-09-12/13 dos sesiones ejecutaron la misma `MVC-0385` y tomaron IDs ya usados: es el bug
  que este repo existe para matar.
- ❌ NEVER una herramienta MCP ni un endpoint que acepte identidad (`quien`) o `proyecto` por
  parámetro. ✅ ALWAYS el usuario es el `sub` del JWT de syntroAuth y, en `/mcp`, usuario **y**
  proyecto salen del PAT — sin eso cualquier sesión libera el claim del otro o lee otro proyecto
  (decisión 2026-09-13, `docs/PLAN-PASO-1.md`).
- ❌ NEVER una query sobre `id_sequences`, `id_reservations`, `claims`, `suggestions` o
  `access_tokens` sin `project_id` en el `WHERE`. ✅ ALWAYS fail-closed: sin proyecto resuelto no
  hay query — es multi-proyecto desde la primera migración, y una fuga entre proyectos es el bug
  inaceptable aunque ocurra una vez.
- ❌ NEVER usuarios, contraseñas ni proveedores OAuth propios. ✅ ALWAYS syntroAuth autentica y
  esta app autoriza (regla de la suite, `syntroAuth/_docs/PROPUESTA_ROL_POR_APP.md`) — un IdP que
  se duplica en cada app es el problema que syntroAuth existe para evitar.
- ❌ NEVER un PAT, `DATABASE_URL` ni un `.env` con valores en el repo; ❌ NEVER guardar el secreto
  de un PAT, solo su hash. ✅ ALWAYS variables en Railway y `.env.example` sin valores — el MCP es
  remoto y el PAT es la única barrera.
- ❌ NEVER agregar una pieza facturable (worker, cron, segundo contenedor) sin decisión escrita en
  `docs/CONTEXTO-INICIAL.md`. ✅ ALWAYS todo en el único proceso `apps/api` — el objetivo de
  costo es un servicio + un Postgres (2026-09-13).
- ❌ NEVER editar una migración ya aplicada ni escribir una no idempotente. ✅ ALWAYS migración
  nueva en `db/migrations`, corre al arranque — Railway redeploya en cada push y despierta el
  proceso muchas veces.
- ❌ NEVER una reserva de ID o un claim fuera de una transacción. ✅ ALWAYS `UPDATE … RETURNING` /
  `INSERT … ON CONFLICT` en una transacción, con su test de concurrencia (N llamadas en paralelo →
  N resultados distintos) — la atomicidad ES el producto, no una optimización.
- ❌ NEVER afirmar el comportamiento de una API/servicio externo (Railway, rmcp, sqlx, Postgres,
  GitHub) como hecho desde una conclusión propia. ✅ ALWAYS anclarlo a procedencia citable — doc
  oficial/SDK (`oficial`) o reproducción propia in-repo (`empírico`); si no se puede validar, es
  `hipótesis` con `TODO: validar` **y su fila en el registro de incógnitas (`docs/TODO.md`
  § "Decisiones abiertas") en el mismo commit** — el TODO es el ancla, el registro es el índice.

## 5. Gate de salida (copiar en cada skill)

```
- [ ] cargo clippy --all-targets -- -D warnings ... verde (o "no existe el crate todavía")
- [ ] cargo test ................................. verde (o "no existe el crate todavía")
- [ ] node scripts/chasis-check.mjs .............. verde (protege `.claude/` — lo que ningún otro gate mira)
- [ ] node scripts/contexto-check.mjs ............ verde (las reglas no mienten sobre el repo)
- [ ] node scripts/docs-linkcheck.mjs ............ sin problemas nuevos contra HEAD
- [ ] specs viejos revisados: ningún test pasa en verde mintiendo sobre el comportamiento real
- [ ] estado consolidado en docs/TODO.md (la ficha/tarea movida a su lugar terminal)
- [ ] continuous-learning: ¿la sesión dejó un aprendizaje no trivial? → /td-learn antes de cerrar
      (o declarar "nada que persistir")
```

> **Ritual de cierre (obligatorio):** el último ítem no es opcional. Si un gate de esta lista no
> pudo correr, se dice explícito — no se finge verde.

## 6. Convenciones de docs

`docs/` es la bóveda, chica a propósito. Reglas:
- **Índices**: `README.md` (raíz) y `docs/TODO.md`. Todo doc nuevo se enlaza desde uno de los dos
  en el mismo commit — un huérfano es bug (`docs-linkcheck` lo marca; `docs-ratchet` bloquea solo
  lo NUEVO).
- **Links markdown relativos** (`[x](docs/x.md)`), nunca wikilinks ni rutas absolutas: es lo que
  sobrevive en GitHub y en Railway.
- **Un ADR aceptado (`docs/DECISIONS/`) no se reescribe**: el aprendizaje nuevo es un nodo nuevo
  que lo enlaza. `docs/CONTEXTO-INICIAL.md` es punto de partida histórico, ❌ NEVER estado: el
  estado vive en `docs/TODO.md`.

## 7. Proof-of-Bug — clasificar Bug vs Sugerencia

→ **`.claude/proof-of-bug.md`**. El filtro por impacto observable y la regla de evidencia
ejecutable. Lo referencian las skills que reportan o reproducen hallazgos, en el punto de uso.

## 8. Ingeniería de contexto

→ **`.claude/contexto-agente.md`**. Leerlo al auditar o reestructurar capas de contexto (rules,
skills, memoria, compactación): los 4 criterios de auditoría, el filtro de restricciones y las
reglas de caché/compactación. No hace falta para ninguna otra tarea.
