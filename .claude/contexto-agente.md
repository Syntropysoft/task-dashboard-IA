<!-- chasis-kit v4 -->
# Ingeniería de contexto del agente

> Se lee **solo** al auditar, escribir o reestructurar capas de contexto (rules, skills, memoria,
> prompts de sesión, compactación). No aplica a ninguna otra tarea. Puntero desde la capa
> siempre-cargada en `rules/` (el nombre de ese archivo lo elige cada repo — este genérico no lo
> fija). Genérico, no tocar: se mejora en el kit y se re-sincroniza.
>
> **Procedencia:** apuntes de curso de context engineering, destilados 2026-08-29. Los números
> citados (benchmarks, porcentajes, costos) son del material y NO fueron verificados de primera
> mano → tratarlos como `hipótesis` con dirección correcta, no como cifras exactas.

---

## 1. Los 4 criterios de auditoría de contexto

Cada uno arregla un tipo distinto de falla; una auditoría pasa los cuatro, no uno.

| Criterio      | Decide          | Pregunta de auditoría                                             |
| ------------- | --------------- | ----------------------------------------------------------------- |
| **Relevance** | qué entra       | si quito esta línea, ¿cambia alguna respuesta? Si no, sobra.      |
| **Recency**   | qué sale        | ¿lo viejo puede MORIR, o solo se acumula?                         |
| **Retrieval** | cuándo entra    | ¿tiene que estar SIEMPRE, o solo cuando alguien lo pide?          |
| **Ranking**   | dónde se coloca | ¿lo decisivo está donde más se ve (inicio/final, nunca al medio)? |

- ❌ NEVER asumir que más herramientas/contexto = agente más capaz. ✅ ALWAYS el conjunto MÁS
  CHICO de tokens de alta señal que maximiza el resultado — en el apunte, un modelo falló con 46
  herramientas y funcionó con 19: quitarle opciones lo mejoró.
- ❌ NEVER corregir un error agregando texto encima: el dato viejo y el correcto conviven en el
  mismo contexto y el error se arrastra (−39% reportado en multi-turno con error temprano).
  ✅ ALWAYS que la corrección REEMPLACE — editar la fuente de estado, compactar, arrancar sesión
  nueva — no que se apile.
- ❌ NEVER precargar "por si acaso". ✅ ALWAYS referencia liviana (ruta, link) + traer
  just-in-time; híbrido sano: lo crítico por adelantado, el resto bajo demanda.
- ❌ NEVER enterrar la invariante clave al medio de un bloque largo — la posición media pierde
  15–20 puntos de exactitud (apunte). ✅ ALWAYS lo decisivo al inicio o al final del bloque.

## 2. System prompt: mínimo Y completo

Tres bloques, cada uno con su trampa: **rol** (límites y decisiones reales, no adjetivos tipo
"experto"), **constraints** (la trampa: acumular hasta que se contradicen), **output** (formato y
longitud descritos aunque "parezcan obvios" — nunca lo son).

- ❌ NEVER una restricción sin caso real nombrable donde el modelo falló sin ella — sin caso es
  corazonada, y cada regla rígida apuesta a que conocés todos los casos límite. ✅ ALWAYS regla
  nacida de falla con fecha. *(Converge con la tesis del chasis por vía independiente —
  refuerzo, no novedad; anotado 2026-08-29.)*
- ❌ NEVER dos instrucciones activas peleando ("nunca comentes" vs "documentá según haga falta"):
  el modelo gasta capacidad resolviendo el conflicto en vez de trabajar. ✅ ALWAYS al agregar una
  regla, buscar con cuál choca; el conflicto se resuelve BORRANDO una, no matizando ambas.
- Sweet spot ("Ricitos de Oro"): el conjunto mínimo que describe COMPLETO el comportamiento.
  Sobreespecificar se rompe con el primer caso imprevisto; subespecificar deja al modelo
  adivinando. Una regla que describe el patrón ("código que se lea como el de alrededor") cubre
  más casos que diez prohibiciones puntuales.

## 3. Recuperación: búsqueda · caché · compactación

**Buscar son tres pasos, no uno:** búsqueda (candidatos) → ranking (relevancia, no solo
similitud) → selección (cuántos entran = decisión de PRESUPUESTO, no de calidad). Los modelos son
casi perfectos en coincidencia literal y se degradan en la semántica ("me llegó roto" no matchea
"daño en tránsito") — diseñar términos de búsqueda con las palabras del que pregunta, no las del
que archiva.

**Caché de prefijos:** el prefijo se construye en orden fijo — herramientas → system prompt →
mensajes — y cambiar un nivel invalida ese nivel y todos los siguientes.

- ❌ NEVER contenido volátil (timestamp, contador, estado del turno) en el system prompt:
  destruye el caché de TODA la conversación en cada petición. ✅ ALWAYS lo volátil viaja en el
  mensaje, no en el prefijo.
- Leer del caché cuesta ~1/10 de reprocesar (apunte) — pero cambia lo que PAGÁS, no el espacio:
  lo cacheado sigue ocupando ventana de contexto.

**Compactar (resumen intencional, no el automático):** primero maximizar RECALL — capturar todo
lo relevante aunque quede largo — y recién después iterar hacia precisión. Al revés se tira lo
irrecuperable: en un resumen no hay vuelta atrás. La versión liviana que más ahorra: borrar
resultados CRUDOS de herramientas viejas conservando el dato extraído (la tabla de hace 20 turnos
no sirve; el número que importaba, sí).

**Cuándo NO traer un dato al contexto:** el modelo ya lo sabe · no cambia la decisión · pesa más
de lo que aporta · ya está intacto en el contexto (el más común: buscar lo mismo dos veces porque
nadie registró que ya estaba).
