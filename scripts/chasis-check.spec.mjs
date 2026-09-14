// chasis-kit v13
/**
 * Tests de `chasis-check.mjs` — con `node:test`, contra un repo Git efímero.
 *
 *   node --test scripts/chasis-check.spec.mjs
 *
 * Existen por una razón concreta: el 2026-08-29 este repo descubrió tres gates que se citaban como
 * garantía y ninguno podía dar rojo (`docs-linkcheck` ciego a `.claude/`, el `pre-commit` sin bit
 * `+x`, y un baseline medido con `git stash` que no excluía lo que medía). Un gate sin un rojo
 * reproducible es una afirmación, no un control — así que el caso 2 de acá ES el bug de `fc066ae`.
 *
 * Mismo enfoque que `docs-linkcheck.spec.mjs`: el checker vive de la interacción entre el disco y
 * el índice de git, así que cada caso arma un repo real. Diferencia: `chasis-check` resuelve su
 * raíz con `git rev-parse --show-toplevel` desde el CWD, así que NO hace falta copiarlo adentro —
 * alcanza con correrlo parado en el fixture.
 */
import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const REPO_ROOT = path.resolve(import.meta.dirname, '..');
const CHECKER = path.join(REPO_ROOT, 'scripts', 'chasis-check.mjs');

const tempDirs = [];
after(() => {
  for (const d of tempDirs) fs.rmSync(d, { recursive: true, force: true });
});

// `-c` por invocación: el fixture es desechable y no debe tocar la config real del usuario.
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

/**
 * Repo efímero con el `.gitignore` REAL del repo: `.claude/*` con whitelist por DIRECTORIO.
 * Esa es la forma exacta que se tragó los archivos de `fc066ae`, así que el fixture la reproduce
 * en vez de inventar una más benigna.
 */
function crearFixture({ versionados = {}, sinVersionar = {} } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-fixture-'));
  tempDirs.push(dir);
  git(dir, ['init', '-q']);

  escribir(dir, '.gitignore', ['.claude/*', '!.claude/commands/', '!.claude/rules/', '!.claude/skills/', ''].join('\n'));
  for (const [rel, contenido] of Object.entries(versionados)) escribir(dir, rel, contenido);

  git(dir, ['add', '-A']);
  git(dir, ['commit', '-qm', 'fixture']);

  // Después del commit: lo que git ignora nunca podría haber entrado al índice.
  for (const [rel, contenido] of Object.entries(sinVersionar)) escribir(dir, rel, contenido);

  return dir;
}

function correr(dir) {
  const r = spawnSync(process.execPath, [CHECKER], { cwd: dir, encoding: 'utf8' });
  return { code: r.status, salida: `${r.stdout}${r.stderr}` };
}

describe('chasis-check', () => {
  test('1 · repo sano: todo `.md` bajo .claude/ versionado y sin punteros → VERDE', () => {
    const dir = crearFixture({
      versionados: {
        '.claude/rules/00-chasis.md': '# Chasis\n\nVer `.claude/skills/mvc-dev/SKILL.md`.\n',
        '.claude/skills/mvc-dev/SKILL.md': '# mvc-dev\n',
      },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
    assert.match(salida, /0 problemas/);
  });

  test('2 · EL BUG DE fc066ae: mudar texto a `.claude/x.md` que .gitignore traga → ROJO', () => {
    const dir = crearFixture({
      versionados: {
        // El chasis se queda apuntando al archivo mudado…
        '.claude/rules/00-chasis.md': '## 1. Anatomía\n\n→ **`.claude/authoring-skills.md`**.\n',
      },
      sinVersionar: {
        // …pero el destino cae bajo `.claude/*` y la whitelist es por DIRECTORIO: git nunca lo ve.
        '.claude/authoring-skills.md': '# Autoría\n\nEl texto que salió del chasis.\n',
      },
    });
    const { code, salida } = correr(dir);

    assert.equal(code, 1, `tenía que dar rojo:\n${salida}`);
    assert.match(salida, /Huérfanos/, 'debe reportarlo como huérfano (chequeo A)');
    assert.match(salida, /authoring-skills\.md/);
    assert.match(salida, /existe en disco pero NO está en git/, 'la forma silenciosa: el puntero apunta a algo que sólo vive en esa máquina');
  });

  test('3 · puntero a un `.claude/…md` que no existe en ningún lado → ROJO', () => {
    const dir = crearFixture({
      versionados: { '.claude/rules/00-chasis.md': 'Ver `.claude/borrado.md` para el detalle.\n' },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /borrado\.md/);
    assert.match(salida, /no existe/);
  });

  test('4 · regresión: un enlace markdown entre backticks es sintaxis MOSTRADA, no una cita', () => {
    // Falso positivo real: mvc-learn documenta el formato de MEMORY.md con `- [Título](archivo.md)`.
    const dir = crearFixture({
      versionados: {
        '.claude/skills/mvc-learn/SKILL.md':
          'Agregá una línea en `MEMORY.md` (`- [Título](archivo.md) — hook`).\n' +
          'Y el ejemplo de enlace relativo: `[chasis](../.claude/rules/00-chasis.md)`.\n',
        '.claude/rules/00-chasis.md': '# Chasis\n',
      },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, `un ejemplo entre backticks no es una referencia:\n${salida}`);
  });

  test('5 · regresión: un archivo en el índice y no en el árbol no puede hacer explotar el gate', () => {
    const dir = crearFixture({
      versionados: { '.claude/rules/00-chasis.md': '# Chasis\n', '.claude/skills/mvc-dev/SKILL.md': '# dev\n' },
    });
    fs.rmSync(path.join(dir, '.claude/skills/mvc-dev/SKILL.md')); // borrado del árbol, sigue en el índice
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
    assert.doesNotMatch(salida, /ENOENT/, 'un gate que crashea no puede reportar');
  });

  test('6 · fuera de un repo git: error claro y código 2, nunca un stack trace', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-sinrepo-'));
    tempDirs.push(dir);
    const { code, salida } = correr(dir);
    assert.equal(code, 2, salida);
    assert.match(salida, /no estás dentro de un repo git/);
    assert.doesNotMatch(salida, /at ModuleJob/, 'sin stack trace');
  });

  test('8 · repo git SIN .claude/ (estado pre-instalación, y el propio kit) → código 2, sin stack trace', () => {
    // Review 2026-08-29: explotaba con "find: .claude: No such file or directory" + stack —
    // exactamente el estado de TODO repo antes de instalar el chasis.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-sinclaude-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    const { code, salida } = correr(dir);
    assert.equal(code, 2, salida);
    assert.match(salida, /no hay .claude/i);
    assert.doesNotMatch(salida, /at ModuleJob|Command failed/, 'sin stack trace');
  });

  test('9 · un .mjs huérfano bajo .claude/ también es huérfano — el modo de falla 4.6 no es exclusivo de .md', () => {
    const dir = crearFixture({
      versionados: { '.claude/rules/00-chasis.md': '# Chasis\n' },
      sinVersionar: { '.claude/hooks/recordatorio.mjs': '// hook util\n' },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /recordatorio\.mjs/);
    assert.match(salida, /Huérfanos/);
  });

  test('10 · hook versionado en 100644 → ROJO (git lo salta en silencio); con +x → VERDE', () => {
    // El caso real: un pre-commit citado como garantía que nunca corrió porque le faltaba el bit.
    const dir = crearFixture({
      versionados: {
        '.claude/rules/00-chasis.md': '# Chasis\n',
        '.githooks/pre-commit': '#!/bin/sh\nexit 0\n',
      },
    });
    const rojo = correr(dir);
    assert.equal(rojo.code, 1, rojo.salida);
    assert.match(rojo.salida, /pre-commit/);
    assert.match(rojo.salida, /ejecutable/i);

    git(dir, ['update-index', '--chmod=+x', '.githooks/pre-commit']);
    const verde = correr(dir);
    assert.equal(verde.code, 0, verde.salida);
  });

  test('11 · rutas-fixture dentro de un *.spec.mjs versionado NO son punteros (drift 2026-08-29)', () => {
    // El bug: los specs del propio kit llevan rutas `.claude/…md` como DATOS de fixture; el
    // chequeo B los escaneaba como citas → 4 falsos positivos en toda instalación del kit.
    // El script ya se protegía a sí mismo (regla del comentario junto a ABS) — el descuido fue
    // no aplicar la misma regla a los specs, que viven de esas rutas y no pueden evitarlas.
    const dir = crearFixture({
      versionados: {
        '.claude/rules/00-chasis.md': '# Chasis\n',
        'scripts/mi-gate.spec.mjs': "const fixture = { '.claude/inventado.md': '# x' };\n",
      },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, `una ruta-fixture en un spec no es una cita:\n${salida}`);
  });

  test('12 · repo Cursor-only (sin .claude/) pero CON .githooks: el gate de hooks TIENE que correr', () => {
    // Descubierto instalando en echeq-motor-riesgo-backend (2026-08-29): un repo .NET cuyo equipo
    // usa Cursor no tiene `.claude/` — y encima lo tiene en .gitignore. El early-exit por
    // "no hay .claude/" apagaba TAMBIÉN los chequeos C y D, que no dependen de `.claude/` en
    // absoluto. Resultado: en todo repo Cursor-only el gate del bit +x era inalcanzable, que es
    // justo el modo de falla 4.4 que ese chequeo existe para cazar. Dos chequeos independientes
    // no pueden compartir una condición de arranque.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-cursoronly-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    escribir(dir, '.githooks/pre-commit', '#!/bin/sh\nexit 0\n');   // versionado en 100644
    git(dir, ['add', '-A']);
    git(dir, ['commit', '-qm', 'fixture']);

    const { code, salida } = correr(dir);
    assert.equal(code, 1, `el hook sin +x tiene que dar rojo aunque no haya .claude/:\n${salida}`);
    assert.match(salida, /pre-commit/);
    assert.match(salida, /ejecutable/i);
  });

  test('13 · repo sin .claude/ Y sin .githooks/ → sigue siendo código 2 (nada que medir)', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-nada-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    const { code, salida } = correr(dir);
    assert.equal(code, 2, salida);
    assert.doesNotMatch(salida, /at ModuleJob|Command failed/, 'sin stack trace');
  });

  test('14 · un .mdc bajo .cursor/ que el .gitignore se traga es un huérfano', () => {
    // La misma trampa que el caso 2, en la carpeta de la otra herramienta. Hallada instalando en
    // echeq-motor-riesgo-frontend (2026-08-29): ese repo ignoraba `.cursor/` ENTERO, así que
    // ninguna regla de Cursor había llegado nunca al equipo. Al destaparlo hay que dejar afuera
    // mcp.json y el estado local, y la forma natural es una whitelist por DIRECTORIO
    // (`.cursor/*` + `!.cursor/rules/`) — que se traga un .mdc suelto exactamente igual.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'chasis-check-cursor-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    escribir(dir, '.gitignore', ['.cursor/*', '!.cursor/rules/', ''].join('\n'));
    escribir(dir, '.cursor/rules/00-chasis.mdc', '---\nalwaysApply: true\n---\n# Chasis\n');
    git(dir, ['add', '-A']);
    git(dir, ['commit', '-qm', 'fixture']);
    escribir(dir, '.cursor/regla-suelta.mdc', '---\nglobs: "**/*.ts"\n---\n# Se ve bien acá y no existe para nadie más\n');
    escribir(dir, '.cursor/mcp.json', '{}\n');   // ignorado a propósito: no es una regla

    const { code, salida } = correr(dir);
    assert.equal(code, 1, `un .mdc fuera de git tiene que dar rojo:\n${salida}`);
    assert.match(salida, /regla-suelta\.mdc/);
    assert.doesNotMatch(salida, /mcp\.json/, 'lo que no es una regla no se reporta');
  });

  test('15 · el checker vendored en scripts/ no puede denunciarse a sí mismo', () => {
    // Bug real (2026-08-29): la cabecera de chasis-check.mjs citaba una ruta de ejemplo bajo
    // .claude/ para explicar el bug que le dio origen. En el repo donde nació, ese archivo
    // existía y no pasaba nada. En TODA instalación nueva con .claude/, el chequeo B lo lee
    // como un puntero roto y el gate arranca en rojo permanente — denunciándose a sí mismo.
    // El propio script ya tenía la regla escrita junto a la regex ABS; no la cumplía.
    const dir = crearFixture({ versionados: { '.claude/rules/00-chasis.md': '# Chasis\n' } });
    fs.mkdirSync(path.join(dir, 'scripts'), { recursive: true });
    fs.copyFileSync(CHECKER, path.join(dir, 'scripts', 'chasis-check.mjs'));
    git(dir, ['add', '-A']);
    git(dir, ['commit', '-qm', 'vendored']);

    const { code, salida } = correr(dir);
    assert.equal(code, 0, `el checker vendored no puede ser su propio hallazgo:\n${salida}`);
  });

  test('7 · la raíz sale del CWD, no de dónde vive el script (gotcha de docs-linkcheck)', () => {
    const dir = crearFixture({
      versionados: { '.claude/rules/00-chasis.md': 'Ver `.claude/falta.md`.\n' },
    });
    // Corrido desde una subcarpeta del fixture: tiene que medir el fixture igual, no este repo.
    const sub = path.join(dir, 'sub', 'dir');
    fs.mkdirSync(sub, { recursive: true });
    const r = spawnSync(process.execPath, [CHECKER], { cwd: sub, encoding: 'utf8' });
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stdout, /falta\.md/, 'midió el fixture');
    assert.doesNotMatch(r.stdout, /motor-ventas/, 'NO midió el repo donde vive el script');
  });
});
