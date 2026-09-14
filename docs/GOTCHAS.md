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

**El Dockerfile no puede fijar el target de Rust: Railway construye en amd64 y un Mac Apple
Silicon en arm64.** Con `x86_64-unknown-linux-musl` a mano, el build local falla en el link con
`cc: unrecognized command-line option '-m64'` (el `cc` de la imagen arm64 no lo conoce). El
target se deriva de `TARGETARCH` en el `Dockerfile` (amd64 → x86_64, arm64 → aarch64). *Verificado
2026-09-13:* con el target fijo, `docker build` rompía; derivado, la imagen queda en 8 MB y arranca
en las dos plataformas. Si algún día hace falta la imagen amd64 desde el Mac: `docker build
--platform linux/amd64`.

**sqlx 0.9 rechaza SQL dinámico en compilación y exige `'static` en `Executor::execute`.**
`sqlx::query(&string)` falla con *"dynamic SQL strings should be audited for possible
injections"* (E0277), y `conn.execute(s.as_str())` con *"`sql` does not live long enough"*
(E0597) — el trait pide `E: 'q` con `'q` ligado a la conexión. Para DDL con un nombre que
generamos nosotros (`create database td_test_<uuid>`) la salida es
`sqlx::query(AssertSqlSafe(string_owned))`: el `String` se mueve adentro y desaparecen las dos
trabas. *Verificado 2026-09-13* en `apps/api/tests/common/mod.rs`. ❌ NEVER usar
`AssertSqlSafe` con texto que venga de un usuario: es exactamente la auditoría que desactiva.

**Las migraciones se embeben al compilar (`sqlx::migrate!("../../db/migrations")`).** El
Dockerfile tiene que copiar `db/` al stage de build o el binario compila con cero migraciones y
arranca "al día" sobre una base vacía — sin error. *Verificado 2026-09-13:* el `COPY db db` está
en el Dockerfile y el smoke de la CI arranca la imagen contra un Postgres real.
