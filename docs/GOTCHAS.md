# Gotchas — task-dashboard-IA

Trampas técnicas **estables** de este repo y su stack (Nivel 2 de `/td-learn`). Cada una con el
hecho, cómo se verificó y por qué lo intuitivo está mal. Lo accionable NO va acá: va a
[TODO.md](TODO.md).

---

**Los hooks de git no viajan en el clon y acá no hay `npm install` que los cablee.** El chasis
trae `.githooks/pre-commit` (con `+x` commiteado), pero `core.hooksPath` es config local: un clon
nuevo commitea sin ninguna red hasta que alguien corre `make hooks` (=`git config core.hooksPath
.githooks`). En los repos Node de la familia lo hace el `prepare` de `package.json`; en un repo
Rust no existe ese gancho. *Verificado 2026-09-13:* `git config core.hooksPath` vacío tras el
`git init`. Además, instalar el hook del repo **desactiva entero** el pre-commit global de la
máquina (`~/.config/git/hooks`): git no compone los dos. El del kit reimplementa la única regla
del global (no commitear sobre `main` si existe `develop`).

**Una whitelist `.claude/*` + `!.claude/rules/` se traga los `.md` sueltos de la raíz de
`.claude/`.** La negación es por directorio: `authoring-skills.md`, `proof-of-bug.md` y
`contexto-agente.md` quedaban ignorados y el chasis se instalaba solo en esta máquina.
*Verificado 2026-09-13* con `git check-ignore -v` antes de copiar el kit. El `.gitignore` vigente
ignora solo el estado local (`settings.local.json`, `.continuous-learning.state`).
