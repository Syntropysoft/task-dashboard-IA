---
description: Genera el prompt de sesión para una ficha del backlog, eligiendo el template por TIPO de ficha (bug/implementación vs spike).
argument-hint: "[ID de ficha o nombre de la tarea]"
---

Invocá la skill `td-plan` para generar el prompt de trabajo de: **$ARGUMENTS**.

Corré su pre-flight completo: normalizá el ID antes de rechazarlo (declarando qué resolviste),
verificá si la ficha espera a un tercero ANTES de imprimir, y elegí el template por TIPO
(bug/implementación → `.claude/templates/task-prompt.md`; spike/sugerencia que NO implementa →
`.claude/templates/task-prompt-spike.md`). Si la ficha no existe, avisá y detenete — no inventes.
