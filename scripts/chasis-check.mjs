#!/usr/bin/env node
// chasis-kit v13
/**
 * chasis-check — el gate que le faltaba a `.claude/`.
 *
 * `docs-linkcheck.mjs` valida la bóveda (`docs/`). Nadie validaba el chasis, y por eso
 * el 2026-08-29 un commit pudo sacar §1/§2/§7 de la regla de chasis (bajo rules/, en la carpeta
 * del agente)
 * hacia dos archivos que `.gitignore` tragaba en silencio: en la máquina que los escribió
 * todo se veía bien, y quien clonara recibía tres punteros al vacío.
 *
 * Dos chequeos:
 *   A · HUÉRFANOS      — todo `.md` bajo `.claude/` (y todo `.mdc` bajo `.cursor/`) que está en
 *                        disco tiene que estar en git.
 *   B · PUNTEROS       — toda ruta `.claude/…md` citada desde un archivo versionado tiene que
 *                        existir Y estar en git.
 *
 * El criterio es `git ls-files`, NO el disco: un archivo que git ignora no existe para nadie más.
 *
 * La raíz se resuelve con `git rev-parse --show-toplevel` desde el CWD — decisión explícita
 * (CONTRATO-CHECKER.md §5): siempre se mide el árbol donde estás parado, y se imprime cuál es.
 * Atarla a `import.meta.dirname` mide el árbol equivocado al invocarlo desde otro worktree.
 *
 * OJO: este archivo se escanea a sí mismo, y como suele vivir vendored en `scripts/` de cada
 * repo, ninguna línea de acá puede contener una ruta de ejemplo bajo la carpeta del agente — se
 * reportaría como puntero roto en toda instalación donde ese archivo de ejemplo no exista. Pasó:
 * la cabecera citaba una regla concreta y el gate arrancaba en rojo denunciándose a sí mismo.
 */

import { execSync } from 'node:child_process';
import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import path from 'node:path';

// Listado recursivo en Node puro. NO `find` shelleado: `execSync` en Windows lanza cmd.exe, donde
// `find` puede resolver a C:\Windows\System32\find.exe —un buscador de TEXTO— y el gate explota
// en vez de medir. Que ande porque el find de Git gana en el PATH es un accidente de la máquina, y
// un gate no se apoya en un accidente. (Fix de Andrés Pacheco en motor-ventas, 2026-08-30.)
// Devuelve rutas relativas a `dir`, con `/`, para comparar directo contra `git ls-files`.
function listarArchivos(dir, filtro = () => true, base = '', out = []) {
  let entradas;
  try {
    entradas = readdirSync(path.join(dir, base), { withFileTypes: true });
  } catch {
    return out; // ilegible o borrado entre el stat y la lectura: no es un hallazgo
  }
  for (const e of entradas) {
    const rel = base ? `${base}/${e.name}` : e.name;
    if (e.isDirectory()) listarArchivos(dir, filtro, rel, out);
    else if (e.isFile() && filtro(rel)) out.push(rel);
  }
  return out;
}

let ROOT;
try {
  ROOT = execSync('git rev-parse --show-toplevel', { encoding: 'utf8' }).trim();
} catch {
  console.error('chasis-check: no estás dentro de un repo git. Nada que medir.');
  process.exit(2);
}
const git = (cmd) =>
  execSync(`git ${cmd}`, { cwd: ROOT, encoding: 'utf8' }).split('\n').filter(Boolean);

// Dos mitades independientes: A/B miran `.claude/`, C/D miran `.githooks/`. Compartían una
// condición de arranque y eso apagaba el gate de hooks en todo repo Cursor-only — descubierto
// instalando en un repo .NET (2026-08-29) que ni tiene `.claude/` ni lo quiere. Cada mitad corre
// si su carpeta existe; si no existe ninguna, no hay nada que medir: código 2, nunca un stack.
const HOOKS_DIR = '.githooks';
const hayClaude = existsSync(path.join(ROOT, '.claude'));
const hayCursor = existsSync(path.join(ROOT, '.cursor'));
const hayHooks = existsSync(path.join(ROOT, HOOKS_DIR));

if (!hayClaude && !hayCursor && !hayHooks) {
  console.error(`chasis-check: en ${ROOT} no hay .claude/, .cursor/ ni ${HOOKS_DIR}/ — nada que chequear (¿chasis sin instalar?).`);
  process.exit(2);
}

const tracked = new Set(git('ls-files'));

// ── A · huérfanos: .md bajo .claude/ presentes en disco pero fuera de git ──────────────
const onDisk = !hayClaude ? [] : git('ls-files --cached --others --exclude-standard -- .claude')
  .concat(
    // `--others` respeta .gitignore, así que los ignorados no aparecen: los buscamos aparte.
    // También .mjs: el modo de falla 4.6 aplica igual a un hook o un script tragado por el ignore.
    listarArchivos(path.join(ROOT, '.claude'), (rel) => rel.endsWith('.md') || rel.endsWith('.mjs')).map((rel) => `.claude/${rel}`),
  )
  .filter((f) => (f.endsWith('.md') || f.endsWith('.mjs')) && !/\.local\./.test(path.basename(f)));

// `.cursor/` sólo aporta sus `.mdc`: una regla fuera de git no existe para el equipo. El resto
// (mcp.json, estado local) se ignora a propósito y no es un hallazgo.
const cursorEnDisco = !hayCursor ? [] :
  listarArchivos(path.join(ROOT, '.cursor'), (rel) => rel.endsWith('.mdc')).map((rel) => `.cursor/${rel}`);

const huerfanos = [...new Set([...onDisk, ...cursorEnDisco])].filter((f) => !tracked.has(f)).sort();

// ── B · punteros: rutas .claude/…md citadas desde archivos versionados ─────────────────
// Los `*.spec.mjs` quedan fuera: sus rutas `.claude/…md` son DATOS de fixture, no citas — la
// misma regla que ya protegía a este archivo por comentario (ver ABS), aplicada a los specs.
const scan = !hayClaude ? [] : [...tracked].filter((f) => (f.endsWith('.md') || f.endsWith('.mjs')) && !f.endsWith('.spec.mjs'));
const colgados = [];

const ABS = /\.claude\/[A-Za-z0-9_./-]+\.md/g;          // una ruta bajo .claude/ terminada en .md
// Ojo: este archivo también se escanea a sí mismo, así que ningún comentario de acá puede
// contener una ruta de ejemplo bajo .claude/ — se reportaría como puntero roto (pasó al escribirlo).
const REL = /\]\(([^)\s]+\.md)\)/g;                      // [texto](../ruta/relativa.md)

for (const file of scan) {
  // Un archivo puede estar en el índice y no en el árbol (borrado staged, checkout parcial).
  // El gate no puede explotar por eso: lo saltea y sigue midiendo.
  const full = path.join(ROOT, file);
  if (!existsSync(full)) continue;

  const text = readFileSync(full, 'utf8');
  const refs = new Set();

  // Una RUTA entre backticks es una cita (así cita este repo) → ABS mira el texto entero.
  // Un ENLACE markdown entre backticks es sintaxis mostrada como ejemplo → REL ignora el código.
  const sinCodigo = text.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');

  for (const m of text.matchAll(ABS)) refs.add(m[0]);
  for (const m of sinCodigo.matchAll(REL)) {
    const resolved = path.normalize(path.join(path.dirname(file), m[1]));
    if (resolved.startsWith('.claude/')) refs.add(resolved);
  }

  for (const ref of refs) {
    const abs = path.join(ROOT, ref);
    const enDisco = existsSync(abs) && statSync(abs).isFile();
    if (!tracked.has(ref)) {
      colgados.push({ file, ref, motivo: enDisco ? 'existe en disco pero NO está en git' : 'no existe' });
    }
  }
}

// ── C · hooks: si el repo versiona hooks propios, tienen que ser EJECUTABLES ──────────
// Un hook en 100644 no falla: git lo salta con un hint que nadie lee. Es el modo de falla 4.4
// en su forma más silenciosa — el gate existe, se cita como garantía, y nunca corrió.
const sinExec = [];
for (const row of git(`ls-files -s -- ${HOOKS_DIR}`)) {
  const [meta, file] = row.split('\t');
  const mode = meta.split(' ')[0];
  if (mode !== '100755') sinExec.push({ file, mode });
}
const hooksHuerfanos = [];
if (hayHooks) {
  for (const f of listarArchivos(path.join(ROOT, HOOKS_DIR)).map((rel) => `${HOOKS_DIR}/${rel}`)) {
    if (!tracked.has(f)) hooksHuerfanos.push(f);
  }
}

// ── informe ───────────────────────────────────────────────────────────────────────────
const mitades = [hayClaude ? '.claude/' : null, hayCursor ? '.cursor/' : null, hayHooks ? `${HOOKS_DIR}/` : null].filter(Boolean);
console.log(`chasis-check · raíz escaneada: ${ROOT} · mitades activas: ${mitades.join(' + ')}`);

if (huerfanos.length) {
  console.log(`\nA · Huérfanos — archivos del chasis (.claude/, .cursor/) fuera de git: ${huerfanos.length}`);
  for (const f of huerfanos) console.log(`   ${f}`);
  console.log('   → git NO los versiona. Revisá .gitignore: `.claude/*` tiene whitelist por');
  console.log('     DIRECTORIO, así que un archivo suelto en la raíz de .claude/ cae fuera.');
}

if (colgados.length) {
  console.log(`\nB · Punteros rotos — rutas .claude/…md citadas que no están en git: ${colgados.length}`);
  for (const c of colgados) console.log(`   ${c.file} → ${c.ref}  (${c.motivo})`);
}

if (sinExec.length) {
  console.log(`\nC · Hooks versionados SIN bit ejecutable — git los salta en silencio: ${sinExec.length}`);
  for (const h of sinExec) console.log(`   ${h.file} (${h.mode})`);
  console.log('   → fix: chmod +x <hook> && git update-index --chmod=+x <hook> — y verlo CORRER en un commit real.');
}
if (hooksHuerfanos.length) {
  console.log(`\nD · Archivos en ${HOOKS_DIR}/ fuera de git: ${hooksHuerfanos.length}`);
  for (const f of hooksHuerfanos) console.log(`   ${f}`);
}

const total = huerfanos.length + colgados.length + sinExec.length + hooksHuerfanos.length;
if (total === 0) {
  console.log(`\n✅ chasis: ${scan.length} archivos escaneados · 0 problemas.`);
  process.exit(0);
}
console.log(`\n❌ chasis: ${total} problema(s).`);
process.exit(1);
