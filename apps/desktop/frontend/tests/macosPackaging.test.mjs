import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const workspaceRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const workflow = readFileSync(path.join(workspaceRoot, '.github/workflows/desktop-ci.yml'), 'utf8');
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

test('macOS DMGs are checksummed, required by release, and included in aggregate assets', () => {
  assert.match(workflow, /name: OpenChat-macOS-\$\{\{ matrix\.arch \}\}/u);
  assert.match(workflow, /node scripts\/release-integrity\.mjs create "release-artifacts\/macos-/u);
  assert.match(workflow, /- macos-packages/u);
  assert.match(workflow, /name: OpenChat-macOS-x86_64/u);
  assert.match(workflow, /name: OpenChat-macOS-arm64/u);
  assert.match(workflow, /release-integrity\.mjs verify release-artifacts\/macos-x86_64/u);
  assert.match(workflow, /release-integrity\.mjs verify release-artifacts\/macos-arm64/u);
  assert.match(workflow, /release-artifacts\/macos-x86_64 release-artifacts\/macos-arm64/u);
});

test('Tauri macOS bundle targets a DMG with an app icon and ad-hoc signature', () => {
  assert.deepEqual(macosConfig.bundle.targets, ['dmg']);
  assert.deepEqual(macosConfig.bundle.icon, ['icons/icon.icns']);
  assert.equal(macosConfig.bundle.macOS.signingIdentity, '-');
  assert.equal(macosIcon.toString('ascii', 0, 4), 'icns');
  assert.equal(macosIcon.readUInt32BE(4), macosIcon.length);
});
