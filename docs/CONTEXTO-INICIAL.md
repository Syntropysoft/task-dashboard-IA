# task-dashboard-IA — contexto inicial

> Nota de arranque escrita el 2026-09-13, al cerrar una sesión de trabajo en Convertix
> (`motor-ventas`), donde nació la idea. Leela al abrir el primer chat de este proyecto.
> Es un punto de partida para discutir, no un diseño cerrado.

## El problema que resuelve

En Convertix trabajan dos devs (Gabriel y Andrés), cada uno con su agente, sobre el mismo repo:

- **El estado existe pero está disperso.** `docs/BACKLOG.md` (índice de fichas abiertas), un backlog
  por iniciativa en `docs/IN-PROGRESS-BACKLOG/` (con `prioridad-iniciativa` en el frontmatter),
  `docs/REFERENCIA/PENDIENTES-ABIERTOS.md` (hipótesis, decisiones sin tomar, gaps, riesgos aceptados
  con gatillo), `docs/AUDITORIAS/`, `docs/DONE-BACKLOG/`, más Trello para trámites de terceros.
  Responder "¿la seguridad está cerrada?" llevó ~10 búsquedas cruzadas.
- **Chocan IDs y trabajo.** El 2026-09-12/13 los dos ejecutaron la misma ficha (`MVC-0385`) y tomaron
  IDs que el otro ya había usado. El tracker actual (una colección de Firestore, `backlog-mnc`) casi
  no se usaba y **sus reglas abiertas vencen el 2026-09-30**.

## La lección que condiciona el diseño

Convertix ya tuvo un doc que copiaba el orden del backlog en tablas propias (`PLAN-EJECUCION.md`):
se desincronizó 4 veces medidas y hubo que retirarlo. **Un dato, un dueño.**

## Dirección acordada (a validar)

| Dato | Dónde vive | Por qué |
| :--- | :--- | :--- |
| Fichas, estados, prioridades, pendientes, auditorías | **En el repo del proyecto** (markdown + frontmatter). El dashboard los **lee**, no los copia | Viajan con el código y los cuida el gate del repo |
| Quién tiene qué ficha AHORA · reserva de IDs · sugerencias/notas sueltas | **En la base de este proyecto** | El markdown no representa lo que es "en vivo"; es lo que hoy hace a medias Firestore |

Las escrituras de estado de una ficha (cerrarla, moverla) siguen yendo por commit en el repo.

## Piezas

1. **Backend chico** — reserva atómica de IDs, claims (tomar/liberar ficha), sugerencias.
2. **MCP** (el entregable de más valor para el agente). Herramientas propuestas:
   - `reservar_id(prefijo)` — atómica: dos sesiones nunca reciben el mismo número.
   - `tomar_ficha(id, quien)` / `liberar_ficha(id)` — reemplaza al tracker de Firestore.
   - `proxima_ficha()` — ordena por `prioridad-iniciativa` (P0 < P0.5 < … ; `pausada`/`post-go-live`
     no compiten), después por prioridad de ficha (🔴🟠🟡🟢), respetando "Depende de" y saltando lo
     marcado `⛔ Trello`.
   - `estado(tema)` — por ejemplo `seguridad`: fichas abiertas + pendientes + auditorías en una llamada.
   - `sugerir(texto, contexto)` — hallazgos que todavía no son ficha.
3. **Indexador** — lee el repo (índices, frontmatter, tablas de pendientes) y expone el estado.
4. **Frontend del dashboard** — con qué venimos, qué falta, qué hay que revisar, sugerencias.

## MVP sugerido (en orden)

1. Backend + MCP con reserva de IDs y claims → reemplaza Firestore **antes del 2026-09-30**.
2. Indexador de solo lectura sobre el repo.
3. Recién después, el frontend.

## Decisiones abiertas

- ¿Andrés está de acuerdo y lo va a usar? (Si uno no lo usa, se repite lo de Firestore.)
- ~~Hosting~~ **Decidido 2026-09-13: infra propia en Railway, con el mínimo de piezas facturables.**
  Queda fuera del monorepo de Convertix (otra cosa, sin multi-tenant, sin sus invariantes) y del
  crédito de AWS del piloto. Forma:
  - **Un solo servicio** de aplicación: backend + MCP (HTTP) + indexador (job interno) + frontend
    (estáticos) en el mismo proceso. No hay razón para más de un servicio con dos usuarios.
  - **Postgres gestionado de Railway, 1 vCPU / 1 GB.** Se eligió sobre SQLite-en-volumen porque
    la diferencia de costo es chica y evita la migración si esto crece; escalar es cambiar el plan.
  - **App sleeping** activado: el servicio duerme sin tráfico. Por eso el indexador (paso 2) se
    dispara por webhook de push de GitHub, no por timer.
  - El MCP es **remoto** (transporte Streamable HTTP, no stdio): los agentes de los dos devs se
    conectan por red. Auth con un token por dev desde el día uno; la identidad sale del token, no
    de un parámetro.
  - Lenguaje: **Rust** (recomendado; `rmcp` + `axum` + `sqlx`) o .NET Native AOT. Ver
    `docs/PLAN-PASO-1.md`.
- ¿Cómo lee el repo? Clon local, API de GitHub, o webhook de push.
- Prioridad frente a lo urgente de Convertix: vulnerabilidades de dependencias (1 crítica en `next`,
  ficha a abrir como MVC-0406), `MVC-0393` (versión de Graph API, vence 2026-09-24) y el medio de pago
  de Meta (antes del 2026-09-30).
