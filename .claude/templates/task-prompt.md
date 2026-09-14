# TAREA: {{TASK_ID}}

Actúa como un Principal Staff Engineer. Vamos a trabajar en la ficha: {{TASK_ID}}.
Resumen: {{TASK_SUMMARY}}
Config declarada por la ficha (modelo/esfuerzo): {{TASK_MODEL}}

⚠️ ANTES DE EMPEZAR: si la ficha declara modelo/config y la sesión no coincide, decilo y pedí el
cambio ANTES de tocar código — lo eligió el análisis que vio el problema, no la sesión que lo
implementa. Si no declara, asignalo con las convenciones del repo y dejalo escrito en la ficha.

INVARIANTES DE LA SESIÓN:
1. Una tarea = una sesión.
2. Protocolo Proof-of-Bug: test en rojo obligatorio antes de cualquier fix.
3. Las invariantes del chasis (§4) no se negocian; ante conflicto con la ficha, frenar y reportar.

PASOS DE EJECUCIÓN OBLIGATORIOS:
1. ANALIZAR: abrí la ficha y confirmá Steps + archivos clave.
2. REPRODUCIR: test que falla (en rojo) por la causa real, antes de tocar el código.
3. FIX (/td-dev): el fix mínimo que respeta la arquitectura del repo.
4. VALIDAR: rojo → VERDE. Gate completo del chasis §5.
5. COMMIT: test + fix juntos, en verde.
6. CERRAR: ficha a su estado terminal + /td-learn (o declarar "nada que persistir").
