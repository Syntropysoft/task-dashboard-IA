// chasis-kit v1
/**
 * Tests de `docs-ratchet.mjs` — con `node:test`, contra repos Git efímeros.
 *
 *   node --test scripts/docs-ratchet.spec.mjs
 *
 * El ratchet existe porque el pre-commit de MVC-0324 exigía CERO problemas de `docs-linkcheck`,
 * y el cero absoluto no es un veredicto justo: 37 de los 39 "problemas" de esta máquina son
 * referencias a docs local-only (`docs/_LOCAL/…`) que en otra máquina SÍ existen — el veredicto
 * dependía de quién commiteaba. La pregunta correcta de un hook no es "¿está sana la bóveda?"
 * sino "¿ESTE COMMIT la empeora?", y esa sí es independiente de la máquina: se mide el árbol
 * actual, se mide `HEAD`, y los problemas preexistentes se cancelan solos.
 *
 * Cada fixture commitea un **checker SINTÉTICO mínimo** que cumple `CONTRATO-CHECKER.md` (valida
 * wikilinks `[[X]]` entre los .md de `docs/` — 20 líneas). Antes copiaba el checker real de
 * motor-ventas y el spec llegaba MUERTO a cualquier otro repo (ENOENT en 10 de 11 casos, review
 * 2026-08-29): el kit predicaba §4.4 y enviaba un spec que no podía dar verde. El spec prueba el
 * RATCHET — que es lo genérico —; las reglas del checker son asunto de cada repo.
 * El ratchet corre el checker PROPIO de cada árbol (el de `HEAD` desde un worktree efímero) —
 * lección del gotcha `import.meta.dirname`: nunca medir un árbol con el script de otro.
 */
import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const REPO_ROOT = path.resolve(import.meta.dirname, '..');
const RATCHET = path.join(REPO_ROOT, 'scripts', 'docs-ratchet.mjs');

// Checker sintético que honra CONTRATO-CHECKER.md: resumen `N problema(s)` siempre, secciones
// `Título: N`, detalle a 3 espacios, exit 0/1. Sin template literals para poder incrustarlo acá.
const CHECKER_SINTETICO = [
  '#!/usr/bin/env node',
  '// Checker sintético de fixture — cumple CONTRATO-CHECKER.md; un checker real valida lo suyo.',
  "import fs from 'node:fs';",
  "import path from 'node:path';",
  "const ROOT = path.resolve(import.meta.dirname, '..');",
  "const DOCS = path.join(ROOT, 'docs');",
  "const notas = fs.existsSync(DOCS) ? fs.readdirSync(DOCS).filter((f) => f.endsWith('.md')).sort() : [];",
  'const rotos = [];',
  'for (const f of notas) {',
  "  const texto = fs.readFileSync(path.join(DOCS, f), 'utf8');",
  '  for (const m of texto.matchAll(/\\[\\[([^\\]]+)\\]\\]/g)) {',
  "    if (!fs.existsSync(path.join(DOCS, m[1] + '.md'))) rotos.push('docs/' + f + ' → [[' + m[1] + ']]');",
  '  }',
  '}',
  'if (rotos.length) {',
  "  console.log('Wikilinks sin archivo destino: ' + rotos.length);",
  "  for (const r of rotos) console.log('   ' + r);",
  '}',
  "console.log((rotos.length ? '❌' : '✅') + ' docs: ' + notas.length + ' notas · ' + rotos.length + ' problema(s) · 0 aviso(s).');",
  'process.exit(rotos.length ? 1 : 0);',
  '',
].join('\n');

function escribirChecker(dir) {
  fs.mkdirSync(path.join(dir, 'scripts'), { recursive: true });
  fs.writeFileSync(path.join(dir, 'scripts', 'docs-linkcheck.mjs'), CHECKER_SINTETICO);
}

const tempDirs = [];
after(() => {
  for (const d of tempDirs) fs.rmSync(d, { recursive: true, force: true });
});

function git(cwd, args) {
  return execFileSync(
    'git',
    ['-c', 'commit.gpgsign=false', '-c', 'user.email=fixture@convertix.local', '-c', 'user.name=fixture', ...args],
    { cwd, encoding: 'utf8' },
  );
}

function escribir(dir, rel, contenido) {
  const full = path.join(dir, rel);
  fs.mkdirSync(path.dirname(full), { recursive: true });
  fs.writeFileSync(full, contenido);
}

/**
 * Repo efímero con el linkcheck real commiteado y una bóveda mínima.
 * `enHead` entra al commit base; `despues` muta el árbol de trabajo SIN commitear —
 * es "lo que este commit está por hacer".
 */
function crearFixture({ enHead = {}, despues = {} } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-ratchet-fixture-'));
  tempDirs.push(dir);
  git(dir, ['init', '-q']);
  escribirChecker(dir);
  escribir(dir, 'docs/HOME.md', '# Home\n\n[[SANO]]\n');
  escribir(dir, 'docs/SANO.md', '# Sano\n\n[[HOME]]\n');
  for (const [rel, contenido] of Object.entries(enHead)) escribir(dir, rel, contenido);
  git(dir, ['add', '-A']);
  git(dir, ['commit', '-qm', 'base']);
  for (const [rel, contenido] of Object.entries(despues)) escribir(dir, rel, contenido);
  return dir;
}

function correr(dir) {
  const r = spawnSync(process.execPath, [RATCHET], { cwd: dir, encoding: 'utf8' });
  return { code: r.status, salida: `${r.stdout}${r.stderr}` };
}

// El doc con el problema PREEXISTENTE que viaja en HEAD en casi todos los casos:
const ROTO_VIEJO = '# Viejo\n\n[[FANTASMA-PREEXISTENTE]]\n';

describe('docs-ratchet', () => {
  test('1 · bóveda limpia en HEAD y en el árbol → VERDE', () => {
    const { code, salida } = correr(crearFixture());
    assert.equal(code, 0, salida);
  });

  test('2 · EL CASO DE ESTA MÁQUINA: HEAD ya tiene problemas y el commit no agrega ninguno → VERDE', () => {
    // Con el gate viejo ("cero absoluto") esto bloqueaba TODO commit. El ratchet lo deja pasar:
    // el problema es preexistente, no de este commit.
    const dir = crearFixture({
      enHead: { 'docs/VIEJO.md': ROTO_VIEJO },
      despues: { 'docs/NUEVO-SANO.md': '# Nuevo\n\n[[HOME]]\n' },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, `un problema preexistente no es culpa de este commit:\n${salida}`);
  });

  test('3 · el commit AGREGA un problema → ROJO, y el reporte nombra SOLO el nuevo', () => {
    const dir = crearFixture({
      enHead: { 'docs/VIEJO.md': ROTO_VIEJO },
      despues: { 'docs/NUEVO.md': '# Nuevo\n\n[[FANTASMA-NUEVO]]\n' },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, `tenía que dar rojo:\n${salida}`);
    assert.match(salida, /FANTASMA-NUEVO/, 'el problema nuevo se nombra');
    assert.doesNotMatch(salida, /FANTASMA-PREEXISTENTE/, 'el preexistente NO se mezcla en el reporte del bloqueo');
  });

  test('4 · el commit ARREGLA un problema preexistente → VERDE (mejorar nunca bloquea)', () => {
    const dir = crearFixture({
      enHead: { 'docs/VIEJO.md': ROTO_VIEJO },
      despues: { 'docs/VIEJO.md': '# Viejo\n\n[[HOME]]\n' },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
  });

  test('5 · arreglar uno y romper OTRO → ROJO igual: los arreglos no compran permisos', () => {
    // Si sólo comparáramos conteos (2 antes, 2 después) esto pasaría. Se compara el CONJUNTO.
    const dir = crearFixture({
      enHead: { 'docs/VIEJO.md': ROTO_VIEJO },
      despues: {
        'docs/VIEJO.md': '# Viejo\n\n[[HOME]]\n',
        'docs/NUEVO.md': '# Nuevo\n\n[[FANTASMA-NUEVO]]\n',
      },
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, `un conteo neto igual esconde el problema nuevo:\n${salida}`);
    assert.match(salida, /FANTASMA-NUEVO/);
  });

  test('6 · no deja worktrees efímeros colgados, ni en verde ni en rojo', () => {
    const dir = crearFixture({
      enHead: { 'docs/VIEJO.md': ROTO_VIEJO },
      despues: { 'docs/NUEVO.md': '# Nuevo\n\n[[FANTASMA-NUEVO]]\n' },
    });
    correr(dir); // rojo
    correr(crearFixture()); // verde
    const wt = git(dir, ['worktree', 'list']);
    assert.equal(wt.trim().split('\n').length, 1, `quedó un worktree colgado:\n${wt}`);
  });

  test('7 · HEAD sin el checker (repo anterior a la convención) → pasa con AVISO, código 0', () => {
    // No se puede medir "antes": no hay contra qué trinquetear. Fail-open explícito y ruidoso,
    // misma política que gitleaks-sin-Docker en el pre-commit.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-ratchet-sincheck-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    escribir(dir, 'docs/HOME.md', '# Home\n');
    git(dir, ['add', '-A']);
    git(dir, ['commit', '-qm', 'base sin checker']);
    escribirChecker(dir); // recién llega al árbol
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
    assert.match(salida, /HEAD no tiene/i, 'el salto se declara, no se calla');
    // Regresión: este camino salía con process.exit(0) DENTRO del try, que saltea el finally —
    // y el worktree efímero quedaba colgado en el repo del usuario.
    const wt = git(dir, ['worktree', 'list']);
    assert.equal(wt.trim().split('\n').length, 1, `worktree colgado por el early-exit:\n${wt}`);
  });

  test('8 · CON EL ENTORNO DE UN HOOK: GIT_DIR/GIT_INDEX_FILE relativos exportados → funciona igual', () => {
    // La primera corrida real (2026-08-29, dentro del pre-commit) crasheó así: git exporta
    // GIT_INDEX_FILE relativo durante `git commit`, el `worktree add` lo hereda, y dentro del
    // worktree `.git` es un ARCHIVO → "index file open failed: Not a directory". El ratchet tiene
    // que lavar el entorno GIT_* antes de tocar git.
    const dir = crearFixture({ enHead: { 'docs/VIEJO.md': ROTO_VIEJO } });
    const r = spawnSync(process.execPath, [RATCHET], {
      cwd: dir,
      encoding: 'utf8',
      env: { ...process.env, GIT_DIR: '.git', GIT_INDEX_FILE: '.git/index' },
    });
    assert.equal(r.status, 0, `con env de hook tiene que medir igual:\n${r.stdout}${r.stderr}`);
    assert.doesNotMatch(`${r.stdout}${r.stderr}`, /Not a directory/);
  });

  test('9 · si medir HEAD falla, la salida es 2 (no pude medir), NUNCA 1 (empeora)', () => {
    // El mismo crash salió con código 1 y el hook mintió: "este commit EMPEORA la bóveda".
    // Una medición fallida jamás puede disfrazarse de veredicto. Se fuerza el fallo con un
    // repo cuyo HEAD no tiene el árbol esperable: acá, un gitdir roto a propósito.
    const dir = crearFixture();
    fs.writeFileSync(path.join(dir, '.git', 'HEAD'), 'ref: refs/heads/no-existe\n');
    const { code, salida } = correr(dir);
    assert.notEqual(code, 1, `medición fallida ≠ empeora:\n${salida}`);
    assert.ok(code === 0 || code === 2, `esperaba 0 (aviso primer-commit) o 2, salió ${code}:\n${salida}`);
    assert.doesNotMatch(salida, /at ModuleJob/, 'sin stack trace crudo');
  });

  test('10 · primer commit del repo (HEAD aún no existe) → pasa con aviso, no crashea', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-ratchet-unborn-'));
    tempDirs.push(dir);
    git(dir, ['init', '-q']);
    escribirChecker(dir);
    escribir(dir, 'docs/HOME.md', '# Home\n');
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
    assert.doesNotMatch(salida, /at ModuleJob/);
  });

  test('11 · fuera de un repo git → código 2 y mensaje claro, sin stack trace', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-ratchet-sinrepo-'));
    tempDirs.push(dir);
    const { code, salida } = correr(dir);
    assert.equal(code, 2, salida);
    assert.doesNotMatch(salida, /at ModuleJob/);
  });
});
