#!/usr/bin/env node
// FORK del default de chasis-kit v13 (2026-09-13) — sin marca a propósito: ya no se re-sincroniza.
// Único cambio: `.claude/` queda fuera de la bóveda. Sus .md los carga el harness del agente por
// ruta, no por enlace, así que "huérfano" ahí no significa nada; y esa carpeta ya tiene sus gates
// (chasis-check para git/punteros, contexto-check para lo que afirman las reglas). Medido en la
// instalación: 12 huérfanos falsos, todos bajo .claude/.
/**
 * docs-linkcheck — ¿la bóveda tiene enlaces rotos o documentos huérfanos?
 *
 * Nació para `echeq-docs`: una bóveda que el 2026-08-04 retiró 308 documentos que describían un
 * sistema inexistente. Ese desastre empieza chico — un enlace que ya no resuelve, un doc que
 * ningún índice enlaza — y envejece en silencio hasta que nadie sabe qué es cierto. Este checker
 * caza la versión chica.
 *
 * Es el DEFAULT genérico del kit: tres chequeos que toda bóveda Markdown necesita. Si tu repo
 * tiene reglas propias (carpetas local-only, convenciones extra), forkealo y sacale la marca
 * `chasis-kit vN` — pasa a ser tuyo. El contrato de salida (CONTRATO-CHECKER.md) es lo único
 * innegociable: es lo que `docs-ratchet.mjs` parsea.
 *
 *   A · ENLACES ROTOS      — [texto](ruta-relativa) cuyo destino no existe en disco.
 *   B · WIKILINKS SIN DESTINO — [[nombre]] que no matchea ningún .md de la bóveda (por basename,
 *                            estilo Obsidian, o por sufijo de ruta).
 *   C · HUÉRFANOS          — .md sin ningún enlace entrante desde OTRO archivo. Los README.md
 *                            están exentos: son índices/puntos de entrada, no hojas.
 *
 * Lo que NO valida: anclas (#sección) dentro del destino, y URLs externas (http/mailto) — fuera
 * de alcance a propósito. Un enlace dentro de un fence o de código inline es sintaxis mostrada,
 * no una referencia.
 *
 * La raíz se resuelve con `git rev-parse --show-toplevel` desde el CWD (contrato §5: decisión
 * explícita, igual que chasis-check y contexto-check de este kit) — se mide el árbol donde estás
 * parado, y se imprime cuál es. El conjunto de archivos sale de `git ls-files` incluyendo
 * untracked no-ignorados: un doc nuevo se valida ANTES de su primer add.
 *
 * Salidas: 0 = sin problemas · 1 = hay problemas · 2 = no pude medir.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

let ROOT;
try {
  ROOT = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
} catch {
  console.error('docs-linkcheck: no estás dentro de un repo git. Nada que medir.');
  process.exit(2);
}

// -z: rutas crudas separadas por NUL — sin esto, git "quotea" los nombres con acentos o espacios
// (los PDF de COELSA) y el path que llega acá no existe en disco.
const archivos = [...new Set(
  execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', '*.md'], {
    cwd: ROOT, encoding: 'utf8',
  }).split('\0').filter(Boolean),
)]
  .filter((f) => !/\.local\./.test(path.basename(f)))
  .filter((f) => !f.startsWith('.claude/')) // carpeta del agente: no es bóveda (ver cabecera)
  .filter((f) => existsSync(path.join(ROOT, f))); // en el índice pero borrado del árbol: no explota, se saltea

// Índices para wikilinks: basename (sin .md) → rutas candidatas.
const porBasename = new Map();
for (const f of archivos) {
  const base = path.basename(f, '.md');
  if (!porBasename.has(base)) porBasename.set(base, []);
  porBasename.get(base).push(f);
}

const INLINE = /!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
const WIKI = /!?\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]*)?\]\]/g;
const ESQUEMA = /^[a-z][a-z0-9+.-]*:/i; // http:, https:, mailto:, tel:, …

const rotos = [];
const wikisRotos = [];
// destino absoluto → set de archivos ORIGEN que lo enlazan (el auto-enlace no salva a nadie).
const entrantes = new Map();
const marcarEntrante = (destinoAbs, origen) => {
  if (!entrantes.has(destinoAbs)) entrantes.set(destinoAbs, new Set());
  entrantes.get(destinoAbs).add(origen);
};

for (const file of archivos) {
  const crudo = readFileSync(path.join(ROOT, file), 'utf8');
  // Fences y código inline afuera ANTES de extraer: un enlace ahí es un ejemplo.
  const texto = crudo.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');

  for (const m of texto.matchAll(INLINE)) {
    let destino = m[1];
    if (ESQUEMA.test(destino) || destino.startsWith('#') || destino.startsWith('//')) continue;
    destino = destino.split('#')[0];
    if (!destino) continue;
    try { destino = decodeURIComponent(destino); } catch { /* % literal sin encoding válido: se prueba tal cual */ }

    const abs = destino.startsWith('/')
      ? path.join(ROOT, destino)
      : path.resolve(ROOT, path.dirname(file), destino);
    if (!abs.startsWith(ROOT)) continue; // escapa del repo: fuera de alcance
    if (existsSync(abs)) {
      marcarEntrante(path.normalize(abs), file);
    } else {
      rotos.push(`${file} → ${m[1]}`);
    }
  }

  for (const m of texto.matchAll(WIKI)) {
    // En una tabla markdown el alias se escapa como `\|` (Obsidian): la barra queda pegada al
    // target capturado y `destino\` no matchea nada — falso positivo visto en motor-ventas.
    const nombre = m[1].trim().replace(/\\$/, '');
    if (!nombre) continue;
    // [[nombre]] resuelve por basename; [[carpeta/nombre]] por sufijo de ruta.
    const candidatas = nombre.includes('/')
      ? archivos.filter((f) => f === `${nombre}.md` || f === nombre || f.endsWith(`/${nombre}.md`) || f.endsWith(`/${nombre}`))
      : (porBasename.get(nombre.replace(/\.md$/, '')) ?? []);
    if (candidatas.length === 0) {
      wikisRotos.push(`${file} → [[${nombre}]]`);
    } else {
      for (const c of candidatas) marcarEntrante(path.normalize(path.join(ROOT, c)), file);
    }
  }
}

// ── C · huérfanos ─────────────────────────────────────────────────────────────────────────────
const huerfanos = archivos
  .filter((f) => path.basename(f) !== 'README.md')
  .filter((f) => {
    const abs = path.normalize(path.join(ROOT, f));
    const origenes = entrantes.get(abs);
    return !origenes || (origenes.size === 1 && origenes.has(f));
  })
  .sort();

// ── informe (CONTRATO-CHECKER.md: secciones `Título: N`, detalles a 3 espacios, resumen SIEMPRE) ─
console.log(`docs-linkcheck · raíz: ${ROOT} · ${archivos.length} archivo(s) .md`);

// Dedup antes de reportar: el mismo enlace repetido en un archivo es UN problema. El Set del
// ratchet colapsa duplicados igual — si acá no se dedupe, el resumen dice un número que el
// ratchet nunca va a ver (pasó en la primera corrida real: 255 acá, 237 allá).
const unicos = (xs) => [...new Set(xs)].sort();
const rotosU = unicos(rotos);
const wikisU = unicos(wikisRotos);
if (rotosU.length) {
  console.log(`\nEnlaces rotos — el destino no existe: ${rotosU.length}`);
  for (const r of rotosU) console.log(`   ${r}`);
}
if (wikisU.length) {
  console.log(`\nWikilinks sin destino: ${wikisU.length}`);
  for (const w of wikisU) console.log(`   ${w}`);
}
if (huerfanos.length) {
  console.log(`\nHuérfanos — ningún otro doc los enlaza (¿falta la línea en el índice de su zona?): ${huerfanos.length}`);
  for (const h of huerfanos) console.log(`   ${h}`);
}

const total = rotosU.length + wikisU.length + huerfanos.length;
if (total === 0) {
  console.log(`\n✅ bóveda: ${archivos.length} archivo(s) · 0 problema(s).`);
  process.exit(0);
}
console.log(`\n❌ bóveda: ${total} problema(s).`);
process.exit(1);
