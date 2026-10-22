import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const frontendRoot = path.resolve(__dirname, '..');

test('index.css imports modularized tokens, typography, and animations stylesheets', () => {
  const indexCss = fs.readFileSync(path.join(frontendRoot, 'src', 'index.css'), 'utf-8');

  assert.match(indexCss, /@import "\.\/design-system\/styles\/tokens\.css";/);
  assert.match(indexCss, /@import "\.\/design-system\/styles\/typography\.css";/);
  assert.match(indexCss, /@import "\.\/design-system\/styles\/animations\.css";/);

  assert.match(indexCss, /@source "\.\/";/);
  assert.match(indexCss, /@custom-variant dark/);
});

test('tokens.css defines color, surface, and elevation variables', () => {
  const tokensCss = fs.readFileSync(path.join(frontendRoot, 'src', 'design-system', 'styles', 'tokens.css'), 'utf-8');

  assert.match(tokensCss, /--background:/);
  assert.match(tokensCss, /--navigation:/);
  assert.match(tokensCss, /--surface:/);
  assert.match(tokensCss, /--raised-surface:/);
  assert.match(tokensCss, /--accent:/);
  assert.match(tokensCss, /\.dark\s*\{/);
  assert.match(tokensCss, /@theme\s*\{/);
});

test('typography.css defines base element rules, markdown prose, and code formatting', () => {
  const typographyCss = fs.readFileSync(
    path.join(frontendRoot, 'src', 'design-system', 'styles', 'typography.css'),
    'utf-8',
  );

  assert.match(typographyCss, /@layer base\s*\{/);
  assert.match(typographyCss, /\.conversation-message-content/);
  assert.match(typographyCss, /\.model-readme-markdown/);
});

test('animations.css defines keyframes, micro-interactions, and reduced-motion fallback', () => {
  const animationsCss = fs.readFileSync(
    path.join(frontendRoot, 'src', 'design-system', 'styles', 'animations.css'),
    'utf-8',
  );

  assert.match(animationsCss, /@keyframes app-main-surface-in/);
  assert.match(animationsCss, /@keyframes openchat-content-in/);
  assert.match(animationsCss, /@keyframes openchat-skeleton-shimmer/);
  assert.match(animationsCss, /@media \(prefers-reduced-motion: reduce\)/);
});

test('design-system index.ts exports Badge, Kbd, and Skeleton primitives', () => {
  const indexTs = fs.readFileSync(path.join(frontendRoot, 'src', 'design-system', 'index.ts'), 'utf-8');

  assert.match(indexTs, /export\s*\{\s*Badge/);
  assert.match(indexTs, /export\s*\{\s*Kbd/);
  assert.match(indexTs, /export\s*\{\s*Skeleton/);
});
