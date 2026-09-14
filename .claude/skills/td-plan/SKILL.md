---
name: td-plan
description: Prompt Factory del repo. Dispara al invocar `/td-plan <ID o tarea>` — localiza la ficha/tarea en la fuente de estado, la clasifica por TIPO y vuelca sus datos sobre el template que corresponde, devolviendo en pantalla el prompt de sesión listo para copiar.
---

# /td-plan — ficha del backlog → prompt de sesión

No genera contenido nuevo: **transcribe** el protocolo ya acordado sobre los datos reales de una
ficha, para que abrir una sesión en frío no dependa de que el usuario recuerde el molde.

## Cuándo usar / Cuándo NO
- ✅ Al arrancar una sesión sobre una tarea existente y necesitar el prompt de apertura armado.
- ❌ NO inventa Steps/Criterios que la ficha no tiene — si está incompleta, el prompt sale
  incompleto y se lo advierte.
- ❌ NO ejecuta nada del prompt generado: no abre rama, no corre tests, no toca el backlog.

## Pre-flight (checklist bloqueante)
- [ ] **Normalizar `$ARGUMENTS` ANTES de rechazarlo** (§4.2 de authoring-skills): un ID a medias
  se completa y se busca en TODAS las series/convenciones del repo. Una sola coincidencia →
  resolver y **declarar cuál se tomó**; cero o varias → recién ahí pedir el ID completo.
  ❌ NEVER elegir por criterio propio ante ambigüedad real.
- [ ] `docs/TODO.md` existe y la ficha está ahí. Si no está, decirlo y detenerse — sin
  degradar en silencio a otro doc (si está cerrada en el archivo de terminadas, decirlo explícito).
- [ ] **Clasificar el TIPO de la ficha** (§4.1): ¿implementa código, o es un spike/sugerencia que
  NO implementa? El tipo decide el template: `task-prompt.md` o `task-prompt-spike.md`. El
  template elegido existe; si falta, detenerse — no fabricar uno ad-hoc.
- [ ] **¿La ficha espera a un TERCERO?** (`⛔` en `docs/TODO.md`: espera a Andrés, a Convertix o a un tercero) → NO generar el prompt en
  silencio: decir qué la bloquea, con el estado real si es verificable (o declarando que no lo
  es — nunca inventado), y **qué parte SÍ se puede trabajar igual** (muchas fichas están
  bloqueadas en un solo paso). ❌ NEVER generar como si estuviera libre; ❌ NEVER negarse si el
  usuario igual lo pide.

## Workflow
1. Localizar el bloque de la ficha. 2. Extraer resumen ("por qué", una o dos líneas) + archivos
clave + modelo/config declarado — si falta un campo, se marca `SIN DECLARAR` y se advierte, nunca
se completa a criterio propio. 3. Leer el template del TIPO **tal cual está versionado** (no
parafrasear de memoria). 4. Rellenar los placeholders. 5. Imprimir el prompt en un bloque de
código, listo para copiar.

## Gate de salida (obligatorio)
```
- [ ] ID normalizado: si vino a medias, se declaró qué se resolvió — o se pidió el completo
- [ ] bloqueo por tercero ADVERTIDO antes del prompt, con estado real o "no verificado"
- [ ] template elegido por TIPO, leído del archivo versionado
- [ ] placeholders con datos reales de la ficha, sin invención; faltantes marcados y advertidos
- [ ] prompt impreso en pantalla; nada ejecutado ni escrito en el repo
```
