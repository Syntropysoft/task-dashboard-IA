#!/usr/bin/env node
// chasis-kit v8
/**
 * contexto-check — el gate de la CAPA DE REGLAS del agente (.cursor/rules/ y .claude/rules/).
 *
 * `chasis-check` valida la integridad git del chasis (.claude/ huérfanos, hooks). Nadie validaba
 * lo que las reglas AFIRMAN, y por eso el 2026-08-29 dos repos (`echeq-backend`, `echeq-frontend`)
 * acumularon reglas citando rutas mudadas (`rates/page.tsx`), scripts inexistentes
 * (`migration:run`), reglas ya borradas (`audit-quality.mdc`) y la misma invariante en dos
 * archivos siempre-cargados sin árbitro. La tesis: el contexto no se juzga por lo que pesa sino
 * por cómo se administra — cuándo entra cada regla, quién gana, y si describe estado o deseo.
 *
 * Cuatro chequeos:
 *   A · PUNTEROS   — toda ruta o script citado entre backticks tiene que existir. Resuelve
 *                    extensiones de import TS (`lib/x` → `lib/x.ts`) y respeta la negación en la
 *                    línea ("no existe", "todavía no") — los DOS falsos positivos que cometió el
 *                    auditor humano ese mismo día.
 *   B · REGLAS     — toda regla `.mdc` citada por nombre tiene que existir en la capa.
 *   C · ENTRADA    — cada `.mdc` declara UN modo de entrada: alwaysApply, globs o description.
 *                    Ninguno = nunca entra. alwaysApply:true + globs = dos a la vez.
 *   D · ÁRBITRO    — la misma línea (≥45 chars normalizados) en dos reglas = dos fuentes de
 *                    verdad; cuando una se edite y la otra no, nadie sabrá cuál manda. Exento:
 *                    el encabezado y el separador de una tabla markdown (estructura, no contenido).
 *
 * Códigos: 0 = ok · 1 = BLOQUEA · 2 = no pude medir (sin git o sin capa de reglas).
 */

import { execSync } from 'node:child_process';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import path from 'node:path';

let ROOT;
try {
  ROOT = execSync('git rev-parse --show-toplevel', { encoding: 'utf8' }).trim();
} catch {
  console.error('contexto-check: no estás dentro de un repo git. Nada que medir.');
  process.exit(2);
}

// ── capa de reglas: los dos layouts conocidos ─────────────────────────────────────────
const CAPAS = [
  { dir: '.cursor/rules', ext: '.mdc', frontmatter: true },
  { dir: '.claude/rules', ext: '.md', frontmatter: false },
];
const reglas = [];
for (const capa of CAPAS) {
  const abs = path.join(ROOT, capa.dir);
  if (!existsSync(abs)) continue;
  for (const f of readdirSync(abs)) {
    if (f.endsWith(capa.ext)) reglas.push({ ...capa, file: path.join(capa.dir, f), base: f });
  }
}
if (reglas.length === 0) {
  console.error(`contexto-check: sin .cursor/rules/ ni .claude/rules/ en ${ROOT} — nada que chequear.`);
  process.exit(2);
}

const scripts = (() => {
  try { return Object.keys(JSON.parse(readFileSync(path.join(ROOT, 'package.json'), 'utf8')).scripts ?? {}); }
  catch { return null; } // sin package.json no se pueden juzgar scripts: se saltean, no se inventan
})();

// Solo se juzga lo que se puede RESOLVER sin ambigüedad: rutas ancladas a un directorio que
// existe en la raíz del repo (o al alias `@/`). Un fragmento de convención (`_actions/`,
// `domain/`, `services/x.js` relativo a un feature) no es verificable desde acá — y juzgarlo
// produjo 59 falsos positivos en la primera corrida real (echeq-frontend, 2026-08-29).
const ANCLAS = new Set(
  readdirSync(ROOT, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name),
);
const EXTS = ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.md', '.mdc', '.json'];
const existeRuta = (rel) => {
  const abs = path.join(ROOT, rel.replace(/\/$/, ''));
  if (existsSync(abs)) return true;
  return EXTS.some((e) => existsSync(abs + e)); // import TS sin extensión — el falso positivo de design-tokens
};
// La línea que AFIRMA inexistencia no cita un puntero: documenta un hueco (falso positivo DECISIONS/).
// Se evalúa sobre la línea SIN las citas: si la palabra de negación está DENTRO del backtick
// (`gate-inexistente.mjs`), es parte del nombre, no una afirmación sobre él.
const NIEGA = /no\s+exist|no\s+tien|no\s+hay|inexistente|todav[íi]a\s+no|eliminad|borrad|no\s+(la|lo|los|las)\s+cre/i;

const punteros = []; // A
const huerfanas = []; // B
const entrada = []; // C
const vistos = new Map(); // D: líneaNormalizada → Set<archivo>
const nombresReglas = new Set(reglas.map((r) => r.base));

for (const regla of reglas) {
  const texto = readFileSync(path.join(ROOT, regla.file), 'utf8');
  let cuerpo = texto;

  // ── C · modo de entrada (solo el frontmatter de Cursor tiene ese contrato) ──────────
  const fm = texto.match(/^---\n([\s\S]*?)\n---\n?/);
  if (fm) cuerpo = texto.slice(fm[0].length);
  if (regla.frontmatter) {
    const f = fm?.[1] ?? '';
    const always = /^alwaysApply:\s*true\b/m.test(f);
    const globs = /^globs:/m.test(f);
    const desc = /^description:/m.test(f);
    if (always && globs) entrada.push({ file: regla.file, motivo: 'alwaysApply:true Y globs — dos modos de entrada a la vez; Cursor ignora los globs y la regla parece de zona sin serlo' });
    if (!always && !globs && !desc) entrada.push({ file: regla.file, motivo: 'sin alwaysApply, sin globs y sin description — esta regla NUNCA entra a una sesión' });
  }

  const sinFences = cuerpo.replace(/```[\s\S]*?```/g, '');
  const lineas = sinFences.split('\n');
  // Separador de tabla markdown (`|---|:--|`). Una fila seguida de uno es el ENCABEZADO: estructura,
  // no contenido normativo — dos reglas que documentan zonas distintas con tablas comparables lo
  // comparten sin ser dos fuentes de verdad. Las filas de DATOS sí se comparan (spec 16 y 17).
  const ES_SEPARADOR = /^\s*\|[\s:|-]+\|\s*$/;
  for (const [i, linea] of lineas.entries()) {
    // ── D · árbitro: línea idéntica en dos reglas ───────────────────────────────────
    const estructuraDeTabla = ES_SEPARADOR.test(linea) || ES_SEPARADOR.test(lineas[i + 1] ?? '');
    const norm = linea.replace(/[`*_>#❌✅·—-]/g, ' ').replace(/\s+/g, ' ').trim().toLowerCase();
    if (norm.length >= 45 && !estructuraDeTabla) {
      if (!vistos.has(norm)) vistos.set(norm, new Set());
      vistos.get(norm).add(regla.file);
    }

    if (NIEGA.test(linea.replace(/`[^`\n]+`/g, ''))) continue; // A no aplica: la línea documenta una ausencia

    for (const m of linea.matchAll(/`([^`\n]+)`/g)) {
      const cita = m[1].trim();
      // ── B · regla citada por nombre ───────────────────────────────────────────────
      if (/^[a-z0-9][a-z0-9-]*\.mdc$/.test(cita)) {
        if (!nombresReglas.has(cita)) huerfanas.push({ file: regla.file, ref: cita });
        continue;
      }
      // ── A · scripts npm citados ───────────────────────────────────────────────────
      for (const s of cita.matchAll(/\b(?:pnpm|npm|yarn)\s+run\s+([A-Za-z0-9:._-]+)/g)) {
        if (scripts && !scripts.includes(s[1])) punteros.push({ file: regla.file, ref: `${s[0]}`, motivo: `"${s[1]}" no está en los scripts de package.json` });
      }
      // ── A · rutas repo-relativas ──────────────────────────────────────────────────
      if (/[\s*{<>|]|:\/\/|\.\.\./.test(cita)) continue; // comando, glob, placeholder, URL o elipsis
      const rel = cita.startsWith('@/') ? cita.slice(2) : cita;
      if (!rel.includes('/') || rel.startsWith('/') || rel.startsWith('~') || rel.startsWith('..')) continue;
      if (!cita.startsWith('@/') && !ANCLAS.has(rel.split('/')[0])) continue; // fragmento de convención: no verificable
      if (!existeRuta(rel)) punteros.push({ file: regla.file, ref: cita, motivo: 'no existe (probadas extensiones .ts/.tsx/.js/.mjs/.md)' });
    }
  }
}

const duplicadas = [...vistos.entries()].filter(([, files]) => files.size >= 2);

// ── informe ───────────────────────────────────────────────────────────────────────────
console.log(`contexto-check · raíz: ${ROOT} · ${reglas.length} regla(s)`);

if (punteros.length) {
  console.log(`\nA · Punteros rotos — rutas o scripts citados que no existen: ${punteros.length}`);
  for (const p of punteros) console.log(`   ${p.file} → \`${p.ref}\`  (${p.motivo})`);
  console.log('   → la regla afirma algo que el repo no cumple: corregí la cita o la regla, no al agente.');
}
if (huerfanas.length) {
  console.log(`\nB · Reglas citadas que no existen en la capa: ${huerfanas.length}`);
  for (const h of huerfanas) console.log(`   ${h.file} → ${h.ref}`);
}
if (entrada.length) {
  console.log(`\nC · Modo de entrada mal declarado: ${entrada.length}`);
  for (const e of entrada) console.log(`   ${e.file} — ${e.motivo}`);
}
if (duplicadas.length) {
  console.log(`\nD · La misma línea en dos reglas — dos fuentes de verdad sin árbitro: ${duplicadas.length}`);
  for (const [norm, files] of duplicadas) console.log(`   "${norm.slice(0, 70)}…"\n      en: ${[...files].join(' · ')}`);
  console.log('   → el conflicto se resuelve BORRANDO una copia y dejando un puntero, no manteniendo dos.');
}

const total = punteros.length + huerfanas.length + entrada.length + duplicadas.length;
if (total === 0) {
  console.log(`\n✅ contexto: ${reglas.length} reglas · 0 problemas.`);
  process.exit(0);
}
console.log(`\n❌ contexto: ${total} problema(s).`);
process.exit(1);
