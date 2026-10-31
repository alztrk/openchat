#!/usr/bin/env node
/**
 * Check import path violations based on layered architecture convention.
 * Rules:
 *   @shared  → can only import from @shared
 *   @entities → can only import from @shared
 *   @features → can import from @shared, @entities, @app, and siblings (cross-feature = warning)
 *   @widgets → can import from @shared, @features, @entities
 *   @app     → can import from anywhere
 *   @modules → OLD (deprecated, should be @features)
 *   platform, shell, bootstrap, design-system, localization → @app umbrella
 */
import { readFileSync, readdirSync } from 'node:fs';
import { relative, resolve, sep } from 'node:path';

const SRC_ROOT = resolve(process.cwd(), 'apps/desktop/frontend/src');

const LAYERS = /** @type {const} */ ({
  shared:  { level: 0, allowed: ['shared'] },
  entities: { level: 1, allowed: ['shared'] },
  features: { level: 2, allowed: ['app', 'entities', 'shared'], crossFeature: true },
  widgets: { level: 3, allowed: ['shared', 'features', 'entities'] },
  app:     { level: 4, allowed: ['app', 'features', 'entities', 'widgets', 'shared', 'modules', 'platform', 'shell'] },
});

const APP_UNIVERSE = new Set(['app', 'platform', 'shell', 'bootstrap', 'design-system', 'localization']);

function getLayerKey(relPath) {
  let p = relPath;
  if (p.startsWith('.' + sep)) p = p.slice(2);
  if (p.startsWith('./')) p = p.slice(2);
  const first = p.split(sep)[0];
  return first || null;
}

function getLayerDef(key) {
  if (APP_UNIVERSE.has(key)) return LAYERS.app;
  return LAYERS[key] || null;
}

function getImportLayerKey(importPath) {
  if (importPath.startsWith('@/')) {
    const afterSlash = importPath.slice(2);
    const first = afterSlash.split('/')[0];
    if (first) return first;
  }
  if (importPath.startsWith('@app')) return 'app';
  if (importPath.startsWith('@features')) return 'features';
  if (importPath.startsWith('@entities')) return 'entities';
  if (importPath.startsWith('@widgets')) return 'widgets';
  if (importPath.startsWith('@shared')) return 'shared';
  if (importPath.startsWith('@modules')) return 'modules';
  if (importPath.startsWith('@platform')) return 'platform';
  if (importPath.startsWith('@shell')) return 'shell';
  if (importPath.startsWith('@bootstrap')) return 'bootstrap';
  if (importPath.startsWith('@design-system')) return 'design-system';
  if (importPath.startsWith('@localization')) return 'localization';
  return null;
}

function isAppUmbrella(layer) { return APP_UNIVERSE.has(layer); }

function collectFiles(dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const full = resolve(dir, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(full));
    else if (entry.isFile() && (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx'))) files.push(full);
  }
  return files;
}

function extractImportPaths(source) {
  const seen = new Set();
  const specifiers = [];
  const staticRe = /import\s+(?:type\s+)?(?:[\w{}\s*,\n\r]+\s+from\s+)?['"](@[^'"]+)['"]/g;
  let m;
  while ((m = staticRe.exec(source)) !== null) { const path = m[1]; if (!seen.has(path)) { seen.add(path); specifiers.push(path); } }
  const dynamicRe = /import\(\s*['"](@[^'"]+)['"]/g;
  while ((m = dynamicRe.exec(source)) !== null) { const path = m[1]; if (!seen.has(path)) { seen.add(path); specifiers.push(path); } }
  return specifiers;
}

function checkImport(sourcePath, importPath) {
  const relSource = relative(SRC_ROOT, sourcePath);
  const sourceLayerKey = getLayerKey(relSource);
  if (!sourceLayerKey) return null;
  const sourceDef = getLayerDef(sourceLayerKey);
  if (!sourceDef) return null;
  const targetLayerKey = getImportLayerKey(importPath);
  if (!targetLayerKey) return null;
  const importDisplay = importPath.replace(/\.tsx?$/, '');

  if (targetLayerKey === 'modules') {
    return { severity: 'info', source: relSource, importPath: importDisplay, message: 'migrate to @features/*' };
  }
  if (sourceLayerKey === 'shared') {
    if (!LAYERS.shared.allowed.includes(targetLayerKey) && !isAppUmbrella(targetLayerKey)) {
      return { severity: 'error', source: relSource, importPath: importDisplay, message: `layer violation (@shared → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'entities') {
    if (!LAYERS.entities.allowed.includes(targetLayerKey) && !isAppUmbrella(targetLayerKey)) {
      return { severity: 'error', source: relSource, importPath: importDisplay, message: `layer violation (@entities → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'features') {
    if (targetLayerKey === 'features') {
      return { severity: 'warning', source: relSource, importPath: importDisplay, message: 'cross-feature import (discouraged)' };
    }
    if (!LAYERS.features.allowed.includes(targetLayerKey) && !isAppUmbrella(targetLayerKey)) {
      return { severity: 'error', source: relSource, importPath: importDisplay, message: `layer violation (@features → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'widgets') {
    if (!LAYERS.widgets.allowed.includes(targetLayerKey) && !isAppUmbrella(targetLayerKey)) {
      return { severity: 'error', source: relSource, importPath: importDisplay, message: `layer violation (@widgets → @${targetLayerKey})` };
    }
    return null;
  }
  return null;
}

function main() {
  const files = collectFiles(SRC_ROOT);
  const errors = [], warnings = [], infos = [];
  for (const file of files) {
    let source;
    try { source = readFileSync(file, 'utf-8'); } catch { continue; }
    const importPaths = extractImportPaths(source);
    for (const imp of importPaths) {
      const result = checkImport(file, imp);
      if (!result) continue;
      if (result.severity === 'error') errors.push(result);
      else if (result.severity === 'warning') warnings.push(result);
      else infos.push(result);
    }
  }
  console.log(`\nChecking imports in src/...\n`);
  if (errors.length > 0) { for (const e of errors) console.log(`\ud83d\udd34 ${e.source} \u2192 ${e.importPath} (${e.message})`); console.log(); }
  if (warnings.length > 0) { for (const w of warnings) console.log(`\ud83d\udfe1 ${w.source} \u2192 ${w.importPath} (${w.message})`); console.log(); }
  if (infos.length > 0) { for (const i of infos) console.log(`\u2139\ufe0f  ${i.source} \u2192 ${i.importPath} (${i.message})`); console.log(); }
  console.log(`Summary: ${errors.length} error${errors.length === 1 ? '' : 's'}, ${warnings.length} warning${warnings.length === 1 ? '' : 's'}, ${infos.length} info ${infos.length === 1 ? 'message' : 'messages'}\n`);
  if (errors.length > 0) process.exit(1);
}

main();
