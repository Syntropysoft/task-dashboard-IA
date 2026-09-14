---
name: td-dev
description: Asistente de implementación del repo. Dispara al implementar una ficha, módulo o feature. Obliga al pre-flight (leer la ficha y ubicar la zona afectada antes de tocar código) y al gate de CI real al cierre, incluyendo reescribir specs que queden mintiendo.
---

# /td-dev — implementar con pre-flight y gate real

## Cuándo usar / Cuándo NO
- ✅ Al implementar una ficha o feature con alcance definido.
- ❌ NO para spikes/sugerencias que declaran que no implementan — esos usan su propio template y
  su entregable es documental.
- ❌ NO arranca sin ficha o alcance escrito: "hacé que ande" no es un alcance.

## Pre-flight (checklist bloqueante)
- [ ] Leer la ficha completa en `docs/TODO.md`: Steps, criterios, archivos clave.
- [ ] Ubicar la zona afectada en la arquitectura real (un solo proceso: `apps/api` — handlers HTTP en `/mcp` (rmcp), `/api`, `/health`; dominio en
  módulos por agregado: `ids`, `claims`, `suggestions`; `db/migrations` para el esquema) — leyendo el código, no suponiendo.
- [ ] Si la ficha es un BUG: el test en rojo va PRIMERO (protocolo de `.claude/proof-of-bug.md`
  §2) — reproducir la falla por su causa real antes de tocar el fix.
- [ ] Verificar el estado del árbol (`git status`, rama correcta) antes de afirmar nada sobre él.

## Workflow
1. Test en rojo (si es bug) o spec del comportamiento nuevo.
2. Implementación mínima que respeta las invariantes del chasis §4 — ante conflicto entre la
   ficha y una invariante, se FRENA y se reporta; no se improvisa una excepción.
3. Gate completo: `make gate` (clippy → test → chasis/contexto/docs). Si un gate no puede correr (falta infra),
   decirlo explícito — no fingir verde.
4. Revisar los specs viejos tocados: ninguno puede quedar en verde mintiendo sobre el
   comportamiento real.
5. Commit con test + fix juntos, en verde. Nunca un rojo suelto al tronco.
6. Cierre: ficha movida a su estado terminal + ritual `/td-learn`.

## Invariantes del repo (❌ NEVER / ✅ ALWAYS)
- Las del §4 de la capa siempre-cargada (`rules/`), sin re-explicarlas acá.
- ❌ NEVER declarar "listo" sin que el gate haya corrido. ✅ ALWAYS el gate es la prueba.

## Gate de salida (obligatorio)
Copiar el gate del chasis §5 y sumarle lo específico de la tarea.
