import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const workflow = readFileSync(path.join(workspaceRoot, '.github/workflows/desktop-ci.yml'), 'utf8').replaceAll('\r\n', '\n');
const macosConfig = JSON.parse(
  readFileSync(path.join(workspaceRoot, 'apps/desktop/tauri/tauri.macos.conf.json'), 'utf8'),
);
const macosIcon = readFileSync(path.join(workspaceRoot, 'apps/desktop/tauri/icons/icon.icns'));

test('macOS packages use native Intel and Apple Silicon runners and targets', () => {
  assert.match(workflow, /macos-15-intel/u);
  assert.match(workflow, /runner: macos-15\s/u);
  assert.match(workflow, /x86_64-apple-darwin/u);
  assert.match(workflow, /aarch64-apple-darwin/u);
});

test('macOS DMGs upload to the draft and both architectures are required before publication', () => {
  const macosJob = workflow.split('\n  macos-packages:\n')[1]?.split(/\n {2}[a-z][a-z0-9-]*:\n/u)[0] ?? '';
  const releaseJob = workflow.split('\n  release:\n')[1] ?? '';
  const publishWorkflow = readFileSync(
    path.join(workspaceRoot, '.github/workflows/publish-tested-release.yml'),
    'utf8',
  );

  assert.match(macosJob, /name: Upload macOS package to the draft release/u);
  assert.match(macosJob, /gh release upload .*release-artifacts\/macos-/u);
  assert.doesNotMatch(macosJob, /actions\/upload-artifact/u);
  assert.match(releaseJob, /- macos-packages/u);
  assert.match(releaseJob, /Both macOS architecture packages are required/u);
  assert.match(releaseJob, /release-integrity\.mjs aggregate/u);
  assert.match(publishWorkflow, /macos_x64/u);
  assert.match(publishWorkflow, /macos_arm64/u);
  assert.match(publishWorkflow, /length >= 2/u);
});

test('Tauri macOS bundle targets a DMG with an app icon and ad-hoc signature', () => {
  assert.deepEqual(macosConfig.bundle.targets, ['dmg']);
  assert.deepEqual(macosConfig.bundle.icon, ['icons/icon.icns']);
  assert.equal(macosConfig.bundle.macOS.signingIdentity, '-');
  assert.equal(macosIcon.toString('ascii', 0, 4), 'icns');
  assert.equal(macosIcon.readUInt32BE(4), macosIcon.length);
});
