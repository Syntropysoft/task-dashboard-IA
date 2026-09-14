// chasis-kit v13
/**
 * Tests de `docs-linkcheck.mjs` — con `node:test`, contra repos Git efímeros.
 *
 *   node --test scripts/docs-linkcheck.spec.mjs
 *
 * Existen por la razón de siempre, pero con un caso propio: este checker nació para `echeq-docs`,
 * una bóveda que el 2026-08-04 retiró 308 documentos que nadie encontraba y que envejecieron hasta
 * describir un sistema inexistente. Un enlace roto o un doc huérfano es la versión chica de ese
 * mismo problema — y el gate que los caza tiene que verse dar ROJO acá antes de custodiar nada.
 *
 * El checker honra CONTRATO-CHECKER.md (lo que docs-ratchet parsea): exit 0/1/2, resumen
 * `N problema(s)` SIEMPRE (también en verde), secciones `Título: N` con detalles a 3 espacios.
 * Varios casos verifican el contrato mismo, no una regla de la bóveda.
 */
import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

const REPO_ROOT = path.resolve(import.meta.dirname, '..');
const CHECKER = path.join(REPO_ROOT, 'scripts', 'docs-linkcheck.mjs');

const tempDirs = [];
after(() => {
  for (const d of tempDirs) fs.rmSync(d, { recursive: true, force: true });
});

function git(cwd, args) {
  execFileSync(
    'git',
    ['-c', 'commit.gpgsign=false', '-c', 'user.email=fixture@kit.local', '-c', 'user.name=fixture', ...args],
    { cwd, stdio: 'pipe' },
  );
}

function crearFixture(archivos = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-linkcheck-fixture-'));
  tempDirs.push(dir);
  git(dir, ['init', '-q']);
  for (const [rel, contenido] of Object.entries(archivos)) {
    const full = path.join(dir, rel);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, contenido);
  }
  git(dir, ['add', '-A']);
  return dir;
}

function correr(dir) {
  const r = spawnSync(process.execPath, [CHECKER], { cwd: dir, encoding: 'utf8' });
  return { code: r.status, salida: `${r.stdout}${r.stderr}` };
}

describe('docs-linkcheck', () => {
  test('1 · bóveda sana: índice → docs enlazados entre sí → VERDE, y el resumen cumple el contrato', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md) y [b](docs/b.md).\n',
      'docs/a.md': 'Ver [b](./b.md).\n',
      'docs/b.md': 'Vuelta a [a](a.md).\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
    // Contrato §2: el resumen `N problema(s)` es obligatorio TAMBIÉN en verde — sin él,
    // docs-ratchet lee "checker roto" y sale 2.
    assert.match(salida, /0 problema\(s\)/);
  });

  test('2 · enlace relativo a un archivo que no existe → ROJO con archivo origen y destino', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md': 'Ver [detalle](./no-existe.md).\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /Enlaces rotos[^:]*: 1/);
    assert.match(salida, /docs\/a\.md/);
    assert.match(salida, /no-existe\.md/);
  });

  test('3 · un enlace dentro de un fence o de código inline es sintaxis MOSTRADA, no una referencia', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md':
        'El formato es `[Título](archivo-de-ejemplo.md)`.\n\n```md\n[otro](tampoco-existe.md)\n[[ni-este]]\n```\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, `un ejemplo no es una referencia:\n${salida}`);
  });

  test('4 · http(s), mailto y anclas puras se ignoran; un ancla sobre archivo real resuelve', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md) y [b](docs/b.md).\n',
      'docs/a.md': '[web](https://example.com) · [mail](mailto:x@y.z) · [acá](#seccion) · [b](b.md#estados)\n',
      'docs/b.md': '## estados\n[a](./a.md)\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
  });

  test('5 · %20 en el destino resuelve al archivo con espacio (caso real: los PDF de COELSA)', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [swagger](docs/Swagger%20Echeq.json) y [a](docs/a.md).\n',
      'docs/Swagger Echeq.json': '{}',
      'docs/a.md': 'x\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
  });

  test('6 · wikilink resuelto por basename desde otra carpeta → VERDE; sin destino → ROJO', () => {
    const verde = crearFixture({
      'README.md': 'Índice: [n](notas/nota.md) y [t](temas/tema.md).\n',
      'notas/nota.md': 'Ver [[tema]].\n',
      'temas/tema.md': 'Vuelta a [[nota]].\n',
    });
    const v = correr(verde);
    assert.equal(v.code, 0, v.salida);

    const rojo = crearFixture({
      'README.md': 'Índice: [n](notas/nota.md).\n',
      'notas/nota.md': 'Ver [[tema-borrado]].\n',
    });
    const r = correr(rojo);
    assert.equal(r.code, 1, r.salida);
    assert.match(r.salida, /Wikilinks sin destino: 1/);
    assert.match(r.salida, /tema-borrado/);
  });

  test('7 · un .md sin ningún enlace entrante es huérfano → ROJO; los README están exentos (son índices)', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md': 'x\n',
      'docs/suelto.md': 'nadie me enlaza\n',
      'docs/sub/README.md': 'índice de zona, tampoco enlazado — exento\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /Huérfanos[^:]*: 1/);
    assert.match(salida, /docs\/suelto\.md/);
    assert.doesNotMatch(salida, /sub\/README\.md/, 'un README es un índice, no un huérfano');
  });

  test('8 · fuera de un repo git: código 2 y error claro, nunca un stack trace', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-linkcheck-sinrepo-'));
    tempDirs.push(dir);
    const { code, salida } = correr(dir);
    assert.equal(code, 2, salida);
    assert.match(salida, /no estás dentro de un repo git/);
    assert.doesNotMatch(salida, /at ModuleJob/, 'sin stack trace');
  });

  test('9 · enlace a un DIRECTORIO existente es válido (así enlaza el README real de echeq-docs)', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [zona](docs/) y [a](docs/a.md).\n',
      'docs/a.md': 'x\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 0, salida);
  });

  test('10 · un enlace desde el propio archivo no lo salva de ser huérfano (auto-enlace no cuenta)', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md': 'x\n',
      'docs/ego.md': 'Me cito: [yo](./ego.md).\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /docs\/ego\.md/);
  });

  test('11 · un .md nuevo SIN stagear también se escanea (el pre-commit corre antes del add de otros)', () => {
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md': 'x\n',
    });
    git(dir, ['commit', '-qm', 'base']);
    fs.writeFileSync(path.join(dir, 'docs/nuevo.md'), 'Ver [roto](./fantasma.md).\n');
    const { code, salida } = correr(dir);
    assert.equal(code, 1, `el untracked tiene que escanearse:\n${salida}`);
    assert.match(salida, /fantasma\.md/);
  });

  test('12 · el mismo enlace roto repetido en un archivo cuenta UNA vez (drift real: 255 vs 237)', () => {
    // En la primera corrida contra echeq-docs el checker reportó 255 y el ratchet 237: la
    // diferencia eran líneas duplicadas que el Set del ratchet colapsaba. El conteo del resumen
    // tiene que coincidir con lo que el ratchet ve, o el número del informe miente.
    const dir = crearFixture({
      'README.md': 'Índice: [a](docs/a.md).\n',
      'docs/a.md': 'Ver [roto](./x.md) y de nuevo [roto](./x.md) y [[nada]] y otra vez [[nada]].\n',
    });
    const { code, salida } = correr(dir);
    assert.equal(code, 1, salida);
    assert.match(salida, /Enlaces rotos[^:]*: 1\b/);
    assert.match(salida, /Wikilinks sin destino: 1\b/);
    assert.match(salida, /2 problema\(s\)/);
  });

  test('13 · un wikilink con pipe ESCAPADO (\\|, sintaxis de tablas) resuelve igual', () => {
    // Falso positivo real (motor-ventas, 2026-08-30): dentro de una tabla markdown el alias de un
    // wikilink se escapa como \| —sintaxis estándar de Obsidian— y la regex capturaba el target
    // CON la barra invertida (`0010-live-ops-feed-sse\`), que no matchea ningún archivo. El gate
    // del repo declaraba «0 problemas» citando este checker, y medía 1: este.
    const dir = crearFixture({
      'docs/GUIA.md': '| col |\n|---|\n| ver [[destino\\|alias]] |\n',
      'docs/destino.md': '# destino\n\n[volver](./GUIA.md)\n',
    });
    const { code, salida } = correr(dir);
    assert.doesNotMatch(salida, /Wikilinks sin archivo destino/, `el pipe escapado es un alias, no parte del nombre:\n${salida}`);
    assert.equal(code, 0, salida);
  });

});
