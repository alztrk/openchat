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
import { fileURLToPath } from 'node:url';

const scriptDirectory = fileURLToPath(new URL('.', import.meta.url));
const sourceRoot = resolve(scriptDirectory, '../apps/desktop/frontend/src');

const layers = {
  shared: { level: 0, allowed: ['shared'] },
  entities: { level: 1, allowed: ['shared'] },
  features: { level: 2, allowed: ['entities', 'shared'], crossFeature: true },
  widgets: { level: 3, allowed: ['shared', 'features', 'entities'] },
  app: { level: 4, allowed: ['app', 'features', 'entities', 'widgets', 'shared', 'modules', 'platform', 'shell'] },
};

const appUniverse = new Set(['app', 'platform', 'shell', 'bootstrap', 'design-system', 'localization']);
const publicFeatureApis = new Set([
  '@features/app-settings/components',
  '@features/app-settings/stores',
  '@features/chat/components',
  '@features/chat/data',
  '@features/chat/state',
  '@features/design-workspace/data',
  '@features/notifications/components',
  '@features/notifications/data',
  '@features/projects/components',
  '@features/provider-settings',
  '@features/provider-settings/data',
]);
const importAliases = new Map([
  ['@app', 'app'],
  ['@features', 'features'],
  ['@entities', 'entities'],
  ['@widgets', 'widgets'],
  ['@shared', 'shared'],
  ['@modules', 'modules'],
  ['@platform', 'platform'],
  ['@shell', 'shell'],
  ['@bootstrap', 'bootstrap'],
  ['@design-system', 'design-system'],
  ['@localization', 'localization'],
]);

export function scanImports({
  sourceRoot: root = sourceRoot,
  collectFilesFn = collectFiles,
  readFileFn = readFileSync,
} = {}) {
  const errors = [];
  const warnings = [];
  const infos = [];
  let files;

  try {
    files = collectFilesFn(root);
  } catch (error) {
    errors.push({
      severity: 'error',
      source: '.',
      importPath: '<source-tree>',
      message: `could not enumerate source files: ${errorMessage(error)}`,
    });
    return { errors, warnings, infos };
  }

  for (const file of files) {
    const sourcePath = relative(root, file);
    let source;
    try {
      source = readFileFn(file, 'utf-8');
    } catch (error) {
      errors.push({
        severity: 'error',
        source: sourcePath,
        importPath: '<unreadable>',
        message: `could not read source file: ${errorMessage(error)}`,
      });
      continue;
    }

    for (const importPath of extractImportPaths(source)) {
      const result = checkImport(sourcePath, importPath);
      if (!result) continue;
      if (result.severity === 'error') errors.push(result);
      else if (result.severity === 'warning') warnings.push(result);
      else infos.push(result);
    }
  }

  return { errors, warnings, infos };
}

function getLayerKey(relativePath) {
  let normalizedPath = relativePath;
  if (normalizedPath.startsWith('.' + sep)) normalizedPath = normalizedPath.slice(2);
  if (normalizedPath.startsWith('./')) normalizedPath = normalizedPath.slice(2);
  const first = normalizedPath.split(sep)[0];
  return first || null;
}

function getLayerDefinition(key) {
  if (appUniverse.has(key)) return layers.app;
  return layers[key] || null;
}

function getImportLayerKey(importPath) {
  if (importPath.startsWith('@/')) {
    const first = importPath.slice(2).split('/')[0];
    if (first) return first;
  }

  for (const [prefix, layer] of importAliases) {
    if (importPath === prefix || importPath.startsWith(`${prefix}/`)) return layer;
  }
  return null;
}

function collectFiles(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(fullPath));
    else if (entry.isFile() && (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx'))) files.push(fullPath);
  }
  return files;
}

function extractImportPaths(source) {
  const seen = new Set();
  const specifiers = [];
  const patterns = [
    /import\s+(?:type\s+)?(?:[\w{}\s*,\n\r]+\s+from\s+)?['"](@[^'"]+)['"]/gu,
    /export\s+(?:type\s+)?(?:\*\s*(?:as\s+[\w$]+\s*)?|\{[^}]*\})\s+from\s*['"](@[^'"]+)['"]/gu,
    /import\(\s*['"](@[^'"]+)['"]/gu,
  ];

  for (const pattern of patterns) {
    let match;
    while ((match = pattern.exec(source)) !== null) {
      const specifier = match[1];
      if (!seen.has(specifier)) {
        seen.add(specifier);
        specifiers.push(specifier);
      }
    }
  }

  return specifiers;
}

function checkImport(relativeSource, importPath) {
  const sourceLayerKey = getLayerKey(relativeSource);
  if (!sourceLayerKey) return null;
  const sourceDefinition = getLayerDefinition(sourceLayerKey);
  if (!sourceDefinition) return null;
  const targetLayerKey = getImportLayerKey(importPath);
  if (!targetLayerKey) return null;
  const importDisplay = importPath.replace(/\.tsx?$/u, '');

  if (targetLayerKey === 'modules') {
    return { severity: 'info', source: relativeSource, importPath: importDisplay, message: 'migrate to @features/*' };
  }
  if (sourceLayerKey === 'shared') {
    if (!layers.shared.allowed.includes(targetLayerKey)) {
      return { severity: 'error', source: relativeSource, importPath: importDisplay, message: `layer violation (@shared → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'entities') {
    if (!layers.entities.allowed.includes(targetLayerKey)) {
      return { severity: 'error', source: relativeSource, importPath: importDisplay, message: `layer violation (@entities → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'features') {
    if (targetLayerKey === 'features') {
      const sourceParts = relativeSource.split(sep);
      const sourceFeature = sourceParts[0] === 'features' ? sourceParts[1] : null;
      const targetFeature = importPath.startsWith('@features/') ? importPath.split('/')[1] : null;
      if (sourceFeature && targetFeature && sourceFeature !== targetFeature) {
        if (publicFeatureApis.has(importPath)) return null;
        return {
          severity: 'warning',
          source: relativeSource,
          importPath: importDisplay,
          message: `cross-feature import (${sourceFeature} → ${targetFeature})`,
        };
      }
      return null;
    }
    if (!layers.features.allowed.includes(targetLayerKey)) {
      return { severity: 'error', source: relativeSource, importPath: importDisplay, message: `layer violation (@features → @${targetLayerKey})` };
    }
    return null;
  }
  if (sourceLayerKey === 'widgets') {
    if (!layers.widgets.allowed.includes(targetLayerKey)) {
      return { severity: 'error', source: relativeSource, importPath: importDisplay, message: `layer violation (@widgets → @${targetLayerKey})` };
    }
  }
  return null;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function main() {
  const { errors, warnings, infos } = scanImports();
  console.log('\nChecking imports in src/...\n');
  for (const error of errors) console.log(`🔴 ${error.source} → ${error.importPath} (${error.message})`);
  for (const warning of warnings) console.log(`🟡 ${warning.source} → ${warning.importPath} (${warning.message})`);
  for (const info of infos) console.log(`ℹ️  ${info.source} → ${info.importPath} (${info.message})`);
  console.log();
  console.log(`Summary: ${errors.length} error${errors.length === 1 ? '' : 's'}, ${warnings.length} warning${warnings.length === 1 ? '' : 's'}, ${infos.length} info ${infos.length === 1 ? 'message' : 'messages'}\n`);
  if (errors.length > 0) process.exitCode = 1;
}

const invocationPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invocationPath === fileURLToPath(import.meta.url)) main();
