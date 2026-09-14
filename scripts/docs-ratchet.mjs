#!/usr/bin/env node
// chasis-kit v1
/**
 * docs-ratchet — ¿este commit EMPEORA la bóveda? (trinquete sobre `docs-linkcheck.mjs`)
 *
 * El pre-commit de MVC-0324 exigía CERO problemas de linkcheck, y el cero absoluto no es un
 * veredicto justo: los docs local-only (`docs/_LOCAL/…`) existen en unas máquinas y en otras no,
 * así que el mismo commit daba 39 problemas acá y ~2 allá — el gate bloqueaba según QUIÉN
 * commiteaba. La pregunta correcta de un hook no es "¿está sana la bóveda?" sino "¿este commit
 * la empeora?", y esa sí tiene la misma respuesta en cualquier máquina:
 *
 *   1. linkcheck sobre el ÁRBOL ACTUAL  → conjunto de problemas "ahora"
 *   2. linkcheck sobre HEAD             → conjunto de problemas "antes"
 *   3. nuevos = ahora − antes. Si hay nuevos → ROJO (y se reportan SOLO esos). Si no → verde.
 *
 * Los preexistentes se cancelan solos porque las dos corridas suceden en la misma máquina, con
 * las mismas carpetas ausentes. Se compara el CONJUNTO, no el conteo: arreglar un problema no
 * compra permiso para romper otro.
 *
 * "Antes" se mide en un WORKTREE EFÍMERO de HEAD, corriendo el linkcheck PROPIO de ese worktree
 * — nunca el de este árbol (gotcha `import.meta.dirname`, docs/CLAUDE/CLAUDE-GOTCHAS.md: medir un
 * árbol con el script de otro devuelve un número plausible del árbol equivocado). Y nunca
 * `git stash`: no toca los untracked, el "antes" quedaría contaminado con lo que estás midiendo.
 *
 * Salidas: 0 = no empeora · 1 = empeora (bloquea) · 2 = no se pudo medir (que el caller decida).
 */

import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// Corriendo dentro de un hook, git exporta GIT_DIR/GIT_INDEX_FILE RELATIVOS; heredarlos rompe
// cualquier git que corra en otro directorio (dentro del worktree `.git` es un archivo →
// "index file open failed: Not a directory" — pasó en la primera corrida real). Se lava todo GIT_*.
const ENV = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith('GIT_')));

let ROOT;
try {
  ROOT = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8', env: ENV, stdio: ['ignore', 'pipe', 'pipe'] }).trim();
} catch {
  console.error('docs-ratchet: no estás dentro de un repo git. Nada que medir.');
  process.exit(2);
}

const CHECKER_REL = path.join('scripts', 'docs-linkcheck.mjs');

/**
 * Corre el linkcheck propio de `dir` y devuelve el conjunto de problemas como claves
 * `sección :: detalle`. Los AVISOS no cuentan: no bloquean hoy y no deben bloquear acá.
 * El formato que se parsea: encabezado de sección sin indentar terminado en `: N`, detalles
 * indentados con 3 espacios, resumen final `bóveda: … problema(s)`.
 */
function medir(dir) {
  const r = spawnSync(process.execPath, [path.join(dir, CHECKER_REL)], { cwd: dir, encoding: 'utf8', env: ENV });
  const salida = `${r.stdout}${r.stderr}`;
  if (r.status !== 0 && r.status !== 1) {
    return { error: `el checker salió con código ${r.status}:\n${salida}` };
  }
  // El resumen se detecta por su FORMA (`N problema(s)`), no por una palabra del dominio —
  // es lo que exige CONTRATO-CHECKER.md del kit para que el ratchet sirva en cualquier repo.
  if (!/\d+ problema\(s\)/.test(salida)) {
    return { error: `la salida del checker no cumple el contrato (falta el resumen "N problema(s)"):\n${salida}` };
  }

  const problemas = new Set();
  let seccion = null;
  let esAviso = false;
  for (const linea of salida.split('\n')) {
    const encabezado = linea.match(/^(\S.*): \d+$/);
    if (encabezado) {
      seccion = encabezado[1];
      esAviso = /^Avisos/.test(seccion);
      continue;
    }
    const detalle = linea.match(/^ {3}(\S.*)$/);
    if (detalle && seccion && !esAviso) problemas.add(`${seccion} :: ${detalle[1].trim()}`);
  }
  return { problemas };
}

// ── "ahora": el árbol de trabajo (mismo criterio que el pre-commit documenta a propósito) ────
const ahora = medir(ROOT);
if (ahora.error) {
  console.error(`docs-ratchet: no pude medir el árbol actual — ${ahora.error}`);
  process.exit(2);
}

// ── "antes": HEAD, materializado en un worktree efímero ──────────────────────────────────────
try {
  execFileSync('git', ['rev-parse', '--verify', '-q', 'HEAD'], { cwd: ROOT, env: ENV, stdio: 'pipe' });
} catch {
  console.log('docs-ratchet: HEAD todavía no existe (¿primer commit?) — no hay "antes" contra el que trinquetear. Paso con aviso.');
  process.exit(0);
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'docs-ratchet-head-'));
const wt = path.join(tmp, 'head');
let antes;
let headSinChecker = false;
try {
  execFileSync('git', ['worktree', 'add', '--detach', '-q', wt, 'HEAD'], { cwd: ROOT, env: ENV, stdio: 'pipe' });
  if (fs.existsSync(path.join(wt, CHECKER_REL))) {
    antes = medir(wt);
  } else {
    // ⚠️ nada de process.exit() acá adentro: saltearía el finally y dejaría el worktree colgado.
    headSinChecker = true;
  }
} catch (e) {
  // Una medición fallida JAMÁS se disfraza de veredicto: en la primera corrida real este crash
  // salió con código 1 y el hook reportó "este commit EMPEORA la bóveda". Mentira: no se midió.
  console.error(`docs-ratchet: no pude materializar HEAD para medir — ${e.stderr?.toString().trim() || e.message}`);
  process.exit(2);
} finally {
  try {
    execFileSync('git', ['worktree', 'remove', '--force', wt], { cwd: ROOT, env: ENV, stdio: 'pipe' });
  } catch {
    /* si el worktree no llegó a crearse no hay nada que sacar */
  }
  fs.rmSync(tmp, { recursive: true, force: true });
}
if (headSinChecker) {
  console.log('docs-ratchet: HEAD no tiene el checker todavía — no hay "antes" contra el que trinquetear. Paso con aviso.');
  process.exit(0);
}
if (antes.error) {
  console.error(`docs-ratchet: no pude medir HEAD — ${antes.error}`);
  process.exit(2);
}

// ── el trinquete ─────────────────────────────────────────────────────────────────────────────
const nuevos = [...ahora.problemas].filter((p) => !antes.problemas.has(p)).sort();
const arreglados = [...antes.problemas].filter((p) => !ahora.problemas.has(p)).length;

console.log(`docs-ratchet · antes (HEAD): ${antes.problemas.size} · ahora: ${ahora.problemas.size}${arreglados ? ` · arreglados: ${arreglados} 🎉` : ''}`);

if (nuevos.length === 0) {
  console.log('✅ este commit no empeora la bóveda.');
  process.exit(0);
}

console.log(`\n❌ este commit AGREGA ${nuevos.length} problema(s) que HEAD no tenía:`);
for (const p of nuevos) console.log(`   ${p}`);
console.log('\n(los problemas preexistentes de HEAD no bloquean — sólo lo que este commit rompe)');
process.exit(1);
