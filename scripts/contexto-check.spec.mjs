// chasis-kit v8
/**
 * Tests de `contexto-check.mjs` — con `node:test`, contra un repo Git efímero.
 *
 *   node --test scripts/contexto-check.spec.mjs
 *
 * Existen por una razón concreta: el 2026-08-29, auditando a mano dos repos con capa de reglas
 * (`echeq-backend` `.cursor/`, luego `echeq-frontend`), aparecieron las MISMAS fallas en ambos y
 * ningún gate podía verlas: reglas citando rutas que ya no existían (`rates/page.tsx` mudada,
 * `migration:run` inexistente), reglas citando OTRAS reglas ya borradas (`audit-quality.mdc`),
 * componentes nombrados que ningún archivo define, y la misma invariante escrita en dos archivos
 * siempre-cargados sin árbitro. Cada caso de acá ES una de esas fallas, con fecha.
 *
 * También cubre los falsos positivos del auditor humano de ese día — el checker no puede repetir
 * mis propios errores: `lib/design-tokens` reportado roto (era `.ts`: resolver extensiones), y
 * una línea que DICE que una ruta no existe reportada como puntero roto (negación en la línea).
 *
 * Mismo enfoque que `chasis-check.spec.mjs`: repo git real efímero por caso; el checker resuelve
 * su raíz con `git rev-parse --show-toplevel` desde el CWD, así que alcanza con correrlo parado
 * en el fixture.
 */
import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const REPO_ROOT = path.resolve(import.meta.dirname, '..');
const CHECKER = path.join(REPO_ROOT, 'scripts', 'contexto-check.mjs');

const tempDirs = [];
after(() => {
  for (const d of tempDirs) fs.rmSync(d, { recursive: true, force: true });
});

function git(cwd, args) {
  execFileSync(
    'git',
    ['-c', 'commit.gpgsign=false', '-c', 'user.email=fixture@convertix.local', '-c', 'user.name=fixture', ...args],
    { cwd, stdio: 'pipe' },
  );
}

function escribir(dir, rel, contenido) {
  const full = path.join(dir, rel);
  fs.mkdirSync(path.dirname(full), { recursive: true });
  fs.writeFileSync(full, contenido);
  return full;
}

function crearFixture(archivos = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'contexto-check-fixture-'));
  tempDirs.push(dir);
  git(dir, ['init', '-q']);
  for (const [rel, contenido] of Object.entries(archivos)) escribir(dir, rel, contenido);
  return dir;
}

function correr(cwd) {
  const r = spawnSync(process.execPath, [CHECKER], { cwd, encoding: 'utf8' });
  return { code: r.status, out: `${r.stdout}\n${r.stderr}` };
}

// Regla mínima sana: un solo modo de entrada, sin citas rotas.
const REGLA_SANA = ['---', 'description: regla de zona', 'globs: "src/**/*.ts"', '---', '', '# Zona', 'Tocar `src/index.ts` con cuidado.', ''].join('\n');

describe('contexto-check', () => {
  test('1 · verde: reglas sanas, rutas y scripts reales', () => {
    const dir = crearFixture({
      'package.json': JSON.stringify({ scripts: { lint: 'eslint .' } }),
      'src/index.ts': 'export {}\n',
      '.cursor/rules/zona.mdc': REGLA_SANA,
      '.cursor/rules/chasis.mdc': ['---', 'description: chasis', 'alwaysApply: true', '---', '', 'Gate: `pnpm run lint` sobre `src/index.ts`.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });

  test('2 · rojo: ruta citada que no existe (rates/page.tsx mudada, 2026-08-29)', () => {
    const dir = crearFixture({
      'app/bank-admin/rates/page.tsx': 'export default () => null\n', // la ruta REAL: se mudó acá
      '.cursor/rules/refactor.mdc': ['---', 'description: refactor', 'globs: "app/**"', '---', '', 'Ejemplo: `app/bank-operator/rates/page.tsx`.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /rates\/page\.tsx/);
  });

  test('3 · verde: import TS sin extensión resuelve a .ts (falso positivo lib/design-tokens, 2026-08-29)', () => {
    const dir = crearFixture({
      'lib/design-tokens.ts': 'export const roleColors = {}\n',
      '.cursor/rules/tokens.mdc': ['---', 'description: tokens', 'globs: "**/*.tsx"', '---', '', 'Usar `@/lib/design-tokens` en el componente que pinta.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });

  test('4 · verde: la línea que DICE que una ruta no existe no es un puntero roto', () => {
    const dir = crearFixture({
      '.cursor/rules/learn.mdc': ['---', 'description: cierre', 'globs: "**/*.md"', '---', '', 'El repo no tiene `docs/DECISIONS/` todavía: se crea con el primero.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });

  test('5 · rojo: script npm citado que no existe (migration:run, echeq-backend 2026-08-29)', () => {
    const dir = crearFixture({
      'package.json': JSON.stringify({ scripts: { lint: 'eslint .' } }),
      '.cursor/rules/release.mdc': ['---', 'description: release', 'globs: "**"', '---', '', 'Aplicar con `pnpm run migration:run` antes del deploy.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /migration:run/);
  });

  test('6 · rojo: regla citada que ya no existe (audit-quality.mdc, 2026-08-29)', () => {
    const dir = crearFixture({
      '.cursor/rules/chasis.mdc': ['---', 'description: chasis', 'alwaysApply: true', '---', '', 'Por tarea: `audit-quality.mdc`.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /audit-quality\.mdc/);
  });

  test('7 · rojo: regla sin ningún modo de entrada — nunca entra a una sesión', () => {
    const dir = crearFixture({
      '.cursor/rules/muerta.mdc': ['---', 'alwaysApply: false', '---', '', '# Regla', 'Texto.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /muerta\.mdc/);
  });

  test('8 · rojo: alwaysApply:true + globs — dos modos de entrada declarados a la vez', () => {
    const dir = crearFixture({
      '.cursor/rules/ambigua.mdc': ['---', 'description: x', 'globs: "**/*.ts"', 'alwaysApply: true', '---', '', 'Texto.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /ambigua\.mdc/);
  });

  test('9 · rojo: la misma línea larga en dos reglas — dos fuentes de verdad sin árbitro', () => {
    const linea = 'Commit: formato ECLB-xxxx Descripcion, sin acentos en el mensaje del commit.';
    const dir = crearFixture({
      '.cursor/rules/chasis.mdc': ['---', 'description: chasis', 'alwaysApply: true', '---', '', linea, ''].join('\n'),
      '.cursor/rules/git.mdc': ['---', 'description: git', 'alwaysApply: true', '---', '', linea, ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /chasis\.mdc/);
    assert.match(out, /git\.mdc/);
  });

  test('10 · exit 2: repo sin capa de reglas — medición imposible ≠ contexto roto', () => {
    const dir = crearFixture({ 'src/a.ts': 'export {}\n' });
    const { code } = correr(dir);
    assert.equal(code, 2);
  });

  test('11 · también escanea .claude/rules/*.md (el layout de Claude Code)', () => {
    const dir = crearFixture({
      'scripts/otro-gate.mjs': 'export {}\n',
      '.claude/rules/00-chasis.md': '# Chasis\n\nVer `scripts/gate-inexistente.mjs`.\n',
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /gate-inexistente\.mjs/);
  });

  test('13 · verde: un fragmento de convención (`_actions/`, `domain/`) no es una ruta desde la raíz', () => {
    // Primera corrida contra echeq-frontend (2026-08-29): 59 "punteros rotos", casi todos
    // fragmentos así. Solo se juzga lo ANCLADO a un directorio real de la raíz (o `@/`).
    const dir = crearFixture({
      'app/x/page.tsx': 'export default () => null\n',
      '.cursor/rules/arq.mdc': ['---', 'description: arq', 'globs: "**/*.ts"', '---', '', 'Las actions van en `_actions/` y el dominio en `domain/`; también `services/x/helper.js` relativo al feature.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });

  test('14 · verde: una ruta con elipsis (`app/.../x`) es un esquema, no una cita verificable', () => {
    const dir = crearFixture({
      'app/x/page.tsx': 'export default () => null\n',
      '.cursor/rules/arq.mdc': ['---', 'description: arq', 'globs: "**/*.ts"', '---', '', 'La UI vive en `app/.../end-user/_ui/`.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });

  test('15 · rojo: la ruta anclada a un dir real de la raíz SÍ se juzga aunque haya fragmentos', () => {
    const dir = crearFixture({
      'app/x/page.tsx': 'export default () => null\n',
      '.cursor/rules/arq.mdc': ['---', 'description: arq', 'globs: "**/*.ts"', '---', '', 'Fragmento `_ui/` ok, pero `app/no-existe/page.tsx` se juzga.', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /app\/no-existe\/page\.tsx/);
  });

  test('16 · verde: el ENCABEZADO de una tabla repetido en dos reglas no es dos fuentes de verdad', () => {
    // Falso positivo real, hallado instalando en echeq-motor-riesgo-backend (2026-08-29):
    // `test-per-method` y `test-per-repository` cubren zonas distintas con técnicas distintas
    // (Moq vs EF In-Memory) — no se pisan en nada. Lo único que compartían era la fila
    // `| Donde se crea el código | Dónde crear/actualizar el test |`, que es ESTRUCTURA de tabla,
    // no contenido normativo. Un gate que bloquea un commit por un encabezado de tabla es el
    // gate que cría lobos: se desactiva y con él se pierden los chequeos que sí valían.
    const tabla = [
      '## Dónde ubicar los tests',
      '',
      '| Donde se crea el código | Dónde crear/actualizar el test |',
      '|-------------------------|----------------------------------|',
    ].join('\n');
    const dir = crearFixture({
      '.cursor/rules/test-per-method.mdc':
        '---\nglobs: "**/Service/**/*.cs"\n---\n' + tabla + '\n| `Service/*.cs` | `Tests/Services/` |\n',
      '.cursor/rules/test-per-repository.mdc':
        '---\nglobs: "**/Repositories/**/*.cs"\n---\n' + tabla + '\n| `Repositories/*.cs` | `Tests/Repositories/` |\n',
    });
    const { code, out } = correr(dir);
    assert.doesNotMatch(out, /dos fuentes de verdad/, `el encabezado de tabla no es contenido:\n${out}`);
    assert.equal(code, 0, out);
  });

  test('17 · rojo: una fila de tabla con CONTENIDO repetida en dos reglas sí es duplicación', () => {
    // El complemento del 16: la exención es para la estructura, no para las filas de datos.
    const fila = '| `MotorRiesgo.Service/*Service.cs` | `MotorRiesgo.Tests/Services/NombreServicioTests.cs` |';
    const dir = crearFixture({
      '.cursor/rules/a.mdc': '---\nglobs: "**/*.cs"\n---\n| x | y |\n|---|---|\n' + fila + '\n',
      '.cursor/rules/b.mdc': '---\nglobs: "**/*.ts"\n---\n| x | y |\n|---|---|\n' + fila + '\n',
    });
    const { code, out } = correr(dir);
    assert.equal(code, 1, out);
    assert.match(out, /dos fuentes de verdad/);
  });

  test('12 · verde: lo que está dentro de un code fence no se escanea (árboles de ejemplo)', () => {
    const dir = crearFixture({
      '.cursor/rules/arq.mdc': ['---', 'description: arq', 'globs: "**/*.ts"', '---', '', '```', 'ruta/que-no-existe/_dtos/api.ts', '```', ''].join('\n'),
    });
    const { code, out } = correr(dir);
    assert.equal(code, 0, out);
  });
});
