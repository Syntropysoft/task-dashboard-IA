<!-- chasis-kit v4 -->
# Proof-of-Bug — clasificar Bug vs Sugerencia y respaldarlo con evidencia

> Lo referencian las skills que **reportan hallazgos** (qa/sec) y la que los **reproduce** en el
> punto de uso. Fuera de una auditoría o de una ficha de bug no aplica, por eso vive fuera de
> `rules/`. Puntero desde el §7 de la capa siempre-cargada en `rules/` (el nombre de ese archivo
> lo elige cada repo — este genérico no lo fija).
> Objetivo: eliminar reportes ambiguos y alucinados **sin** enterrar hallazgos reales.

---

## 1. El filtro de clasificación (por IMPACTO observable, no por existencia de doc)

Ante un hallazgo, clasificar **antes** de redactar la ficha:

- **Bug** = hay un **impacto observable**: pérdida/corrupción de datos o dinero, fuga de datos
  entre usuarios/tenants, caída/500, cómputo incorrecto, o violación de una invariante del repo
  (§4 del chasis). Se puede describir el **fail-path concreto**: entrada/estado → resultado
  incorrecto.
- **Sugerencia / hardening** = el código es **correcto hoy**; el cambio lo hace más robusto,
  limpio o defensivo, pero **no hay un fallo demostrable** que exhibir. (Ej.:
  `Promise.all`→`allSettled` cuando ninguna rama lanza hoy.) Va al backlog **etiquetada como
  Sugerencia**, nunca vestida de Bug.

**El criterio es el impacto, no si un doc lo prohíbe explícitamente.**

- ✅ ALWAYS **citar** el ADR / invariante / línea de doc violada **cuando existe** — fortalece el
  reporte y lo hace verificable.
- ❌ NEVER degradar un bug de correctness/seguridad a "Sugerencia" **porque ningún doc lo
  mencione**. Los docs capturan decisiones e intenciones, **no enumeran cada comportamiento
  incorrecto posible**: ningún ADR dice "no cobres un producto a $0" ni "este endpoint admin
  necesita guard" (en motor-ventas, el modelo de amenazas se escribió *después* del RBAC crítico
  explotado en vivo). **La ausencia de un doc NO es evidencia de que el comportamiento sea
  aceptable.** El default de una auditoría adversarial es exhibir, no minimizar.

## 2. Evidencia ejecutable — dónde y cuándo (no rompe el dry-run ni el tronco)

- La **auditoría** (dry-run) **reporta y deja fichas**; NO exige un test ejecutable por hallazgo
  (muchos piden infra o carreras que no se montan en la sesión de auditoría).
- La **evidencia en rojo** se produce en la **sesión de fix**: test que reproduce el bug en ROJO,
  confirmado que falla por la causa real, y recién entonces el fix (rojo → fix → verde).
  ❌ NEVER commitear un test **en rojo suelto** al tronco (deja el CI en RED hasta que alguien
  tome el fix). ✅ ALWAYS el rojo viaja en la rama de fix y se commitea **junto al fix, en verde**.
- **Verde tras intentar reproducir ≠ falso positivo automático.** Solo tras agotar el **nivel
  correcto** de test (una carrera se prueba con e2e real, no con un unit naive que da verde y
  esconde el bug) se reclasifica **Falso Positivo**: se informa y se retira la ficha con una línea
  de por qué.

## 3. El mismo protocolo aplica al TOOLING

Un gate, un hook o un script de chequeo se construye igual que se arregla un bug — es la lección
más cara de la implementación de referencia:

1. Spec primero, **en rojo** (fixtures = repos git efímeros, el gate corrido como subproceso).
2. Implementar → verde.
3. **Reproducir la falla real** que motiva el gate → tiene que dar ROJO. Si no da rojo, el gate no
   controla nada: reescribilo.
4. Fix → verde; la reproducción queda como test permanente.
5. **Mutación**: romper el gate a propósito y verificar que el spec lo caza — el test correcto, no
   cualquiera.
6. Una corrida real end-to-end. Esperá que enseñe: la primera corrida real siempre ve lo que los
   fixtures no.
