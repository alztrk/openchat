import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const tauriMain = readFileSync(path.join(repositoryRoot, 'apps/desktop/tauri/src/main.rs'), 'utf8');
const rendererMain = readFileSync(
  path.join(repositoryRoot, 'apps/desktop/frontend/src/bootstrap/main.tsx'),
  'utf8',
);

test('the native main window stays hidden until React commits its first view', () => {
  assert.match(tauriMain, /WebviewWindowBuilder::from_config[\s\S]*?\.visible\(false\)/u);
  assert.doesNotMatch(tauriMain, /\.visible\(true\)/u);

  const revealStart = rendererMain.indexOf('function renderAndRevealWindow(');
  const applicationStart = rendererMain.indexOf('\nasync function startApplication', revealStart);
  assert.notEqual(revealStart, -1);
  assert.notEqual(applicationStart, -1);

  const revealFunction = rendererMain.slice(revealStart, applicationStart);
  const reactCommit = revealFunction.indexOf('flushSync(render)');
  const nativeShow = revealFunction.indexOf('.show()');
  const rendererReadyReport = revealFunction.indexOf("invoke('report_webview_renderer_ready')");
  const visibleEvent = revealFunction.indexOf("new Event('openchat-startup-window-visible')");
  assert.notEqual(reactCommit, -1);
  assert.ok(nativeShow > reactCommit);
  assert.ok(rendererReadyReport > nativeShow);
  assert.ok(visibleEvent > rendererReadyReport);

  assert.match(tauriMain, /report_webview_renderer_ready/u);
  assert.match(tauriMain, /recovery_for_callback\.record_failure\(\)/u);
  assert.equal([...rendererMain.matchAll(/renderAndRevealWindow\(/gu)].length, 3);
  assert.match(rendererMain, /React\.createElement\(StartupErrorPage/u);
});
