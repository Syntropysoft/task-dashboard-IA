<!-- chasis-kit v14 -->
# Autoría de skills y commands del chasis

> Se lee **solo** al crear o editar una skill o un command. No aplica a ninguna otra tarea, por eso
> vive fuera de `rules/` (que se carga en toda sesión). Puntero desde la capa siempre-cargada en
> `rules/` (el nombre de ese archivo lo elige cada repo — este genérico no lo fija).
>
> **Procedencia:** molde minado de everything-claude-code (worldflowai) — solo la estructura y los
> 4 bucles; §4 salió de incidentes reales en motor-ventas (la implementación de referencia), con
> fecha, y aplica a cualquier repo.

---

## 1. Anatomía de una skill

Cada skill vive en `.claude/skills/<nombre>/SKILL.md` con este esqueleto fijo:

```markdown
---
name: <nombre>                # kebab-case, == carpeta
description: <cuándo dispara> # una línea; menciona triggers concretos
---

# <Título>

## Cuándo usar / Cuándo NO
## Pre-flight (checklist bloqueante)
## Workflow (pasos numerados)
## Invariantes del repo (❌ NEVER / ✅ ALWAYS)
## Gate de salida (obligatorio)
```

El **command** homónimo en `.claude/commands/<nombre>.md` es un lanzador delgado: recibe
`$ARGUMENTS`, fija el alcance y delega en la skill. La lógica vive en la skill, no se duplica.

## 2. Formato de redacción (obligatorio)

- **`❌ NEVER` / `✅ ALWAYS`**: toda invariante se expresa como par contrastado, no como prosa.
  El "por qué" va en una línea, no en un párrafo.
- **Pre-flight bloqueante**: una skill arranca listando qué leer/verificar ANTES de tocar nada.
  Si un ítem no se cumple, la skill **se detiene y lo reporta — no improvisa**.
- **Pasos numerados**: el workflow es una secuencia `1. → 2. → 3.`, no una descripción.
- **Checklists accionables**: casillas verificables (`[ ]`), no objetivos difusos.
- **Cero bloatware**: si una línea no cambia una decisión del agente, se borra.

## 3. Ruteo y gate

El gate de salida vive en el §5 de la capa siempre-cargada (`rules/`) — al escribir una skill nueva, copiar esa
checklist en su `## Gate de salida` y sumarle lo específico, sin quitarle ítems.

❌ NEVER una `description:` vaga ("skill de seguridad") — es el código de ruteo: sin triggers
concretos la skill nunca dispara y el cuerpo es código muerto.
✅ ALWAYS triggers literales, con las frases que un humano realmente escribe.

❌ NEVER confiar en que el lenguaje natural rutee a la skill: un pre-flight bloqueante sólo protege
si sabés que corriste la skill. ✅ ALWAYS la barra (`/x-…`) es el punto de entrada y el contrato;
la `description:` ayuda, no reemplaza.

❌ NEVER un texto que solo aplica dentro de una skill puesto en `rules/` — ahí se paga en toda
sesión. ✅ ALWAYS en el cuerpo de esa skill, o en un archivo propio con un puntero de una línea.

## 4. Modos de falla del chasis (lo que el molde solo no previene)

§2 hace que una skill se **lea** bien. Estos siete pares hacen que no **ordene lo contrario** de lo
que la tarea manda. Cada uno salió de una falla real (motor-ventas, 2026-08-29/30) — no de teoría.

### 4.1 Clasificar antes de plantillar
❌ NEVER una plantilla única para tipos de trabajo distintos: termina ordenando lo contrario de lo
que la ficha pide, y con toda la autoridad del molde.
✅ ALWAYS el pre-flight clasifica el TIPO de trabajo y recién entonces elige la plantilla.
> Caso: la skill de planificación emitía el molde Proof-of-Bug (`REPRODUCIR → FIX → COMMIT`) sobre
> un spike cuyo step decía literal «este spike NO implementa».

### 4.2 Normalizar la entrada antes de rechazarla
❌ NEVER rechazar `$ARGUMENTS` por formato sin intentar resolverlo — un pre-flight que sólo sabe
pedir «el ID bien escrito» es fricción disfrazada de control.
✅ ALWAYS normalizar primero; si queda ambigüedad REAL, recién ahí preguntar, y declarar qué se
resolvió. Elegir por criterio propio ante ambigüedad real sigue prohibido.

### 4.3 Una regla no puede depender de un artefacto que puede no estar
❌ NEVER una invariante que apunta a un archivo que vive sólo en una rama sin mergear o en una
sola máquina.
✅ ALWAYS el chasis tiene que ser válido en CUALQUIER árbol donde se cargue: si el archivo todavía
no está en el tronco, la regla no está lista para `rules/`.

### 4.4 Un gate que no puede dar rojo no es un gate
❌ NEVER declarar «sin cambios» con una medición que no podría haber detectado el cambio, ni citar
como garantía un gate que nunca viste correr.
✅ ALWAYS verificar que el gate **CORRIÓ y que puede PASAR**, no que exista — y que el baseline
realmente EXCLUYE lo que estás midiendo.
> Casos: `git stash` no toca los untracked (el baseline «limpio» devolvió el mismo número que el
> sucio); un hook commiteado sin bit `+x` que se citaba como «ya no depende de acordarse» y nunca
> corrió — y al encenderlo resultó que tampoco podía pasar (exigía cero absoluto en un estado que
> depende de la máquina → ver el patrón ratchet en el README del kit).

### 4.5 No justifiques una capa con una métrica que no mediste
❌ NEVER mover texto entre capas justificándolo con un ahorro de tokens estimado a ojo.
✅ ALWAYS medir el bloque always-on COMPLETO antes de optimizar una parte, y nombrar la unidad
real — retrabajo evitado — en vez de la cómoda.

### 4.6 Mudar texto a un archivo nuevo no lo versiona
❌ NEVER dar por versionado un archivo nuevo porque existe en tu disco: `.gitignore` puede estar
tragándolo en silencio, y entonces el texto que sacaste de un archivo trackeado no se mudó — se
perdió para todos menos para vos.
✅ ALWAYS después de crear o mudar, correr `git status` y confirmar que el DESTINO aparece; el
gate `scripts/chasis-check.mjs` lo verifica contra `git ls-files` (lo que git ignora no existe
para nadie más).
> Caso: un split del chasis mudó tres secciones a archivos que `.gitignore` tragaba (whitelist por
> DIRECTORIO); quien clonara recibía punteros al vacío, y el linkcheck de docs quedó verde porque
> no mira `.claude/`.

### 4.7 Un puntero mantenido a mano es una afirmación, no un dato
❌ NEVER un índice, un puntero o un agregado que se actualiza a mano y que ningún gate mide. No
deriva ruidosamente: **sigue respondiendo, con el valor viejo**, y por eso nadie se entera.
✅ ALWAYS si el dato se puede derivar del árbol, derivarlo; si tiene que vivir escrito, un
detector compara lo escrito contra lo real. Y el **espacio de nombres del repo** (IDs de ficha,
slugs, claves) necesita su propio chequeo de unicidad — que los links resuelvan no garantiza que
cada identificador sea de uno solo.
> Caso: el puntero «Próximos IDs disponibles» decía `MVC-0288 / FE-0095` cuando el árbol ya usaba
> `MVC-0352 / FE-0111` — 64 IDs de deriva, sin un solo síntoma. El checker de la bóveda validaba
> que los links RESOLVIERAN, no que el namespace fuera coherente, así que el mismo ID se asignó
> más de una vez (dos fichas duplicadas en el tronco, siete colisionando al mergear una rama). Un
> identificador duplicado hace el mismo daño que un puntero al vacío, un nivel más abajo: los dos
> rompen la referencia, pero el duplicado además la rompe **en silencio y hacia atrás**.
