# TAREA: {{TASK_ID}} — spike / sugerencia (NO implementa)

Actúa como un Principal Staff Engineer. Vamos a trabajar en la ficha: {{TASK_ID}}.
Resumen: {{TASK_SUMMARY}}
Config declarada por la ficha (modelo/esfuerzo): {{TASK_MODEL}}

INVARIANTES DE LA SESIÓN:
1. Una tarea = una sesión.
2. **Esta ficha NO implementa.** El entregable es documental: el spike documenta, la ficha decide.
   Si algo exige cambiar código, se ficha aparte — no se toca acá.
3. Procedencia citable: toda afirmación sobre un tercero sale etiquetada `oficial` / `empírico` /
   `hipótesis` con `TODO: validar`. ❌ NEVER una conclusión propia como hecho.
4. Toda incógnita que quede abierta deja su fila en el registro de incógnitas del repo, en el
   MISMO commit.

PASOS DE EJECUCIÓN OBLIGATORIOS:
1. ANALIZAR: abrí la ficha; si un Step depende de un tercero, decilo ANTES de arrancar y acordá
   qué parte SÍ se puede cerrar hoy.
2. INVESTIGAR: cada punto contra doc oficial o reproducción propia in-repo. Lo que no cierre queda
   `hipótesis` EXPLÍCITA — una hipótesis rotulada vale más que una afirmación sin fuente.
3. VOLCAR: el resultado en el doc que la ficha declara como entregable, con URL/`símbolo@versión`
   o script de reproducción.
4. FICHAR: lo que exija código sale como ficha NUEVA enlazada. Acá no se implementa.
5. VALIDAR: el gate de docs del repo, sin problemas nuevos contra el baseline. NO corresponde
   lint/typecheck/test: si se tocó código, no era un spike.
6. COMMIT: sólo documentación. CERRAR: ficha a su estado terminal + /td-learn.
