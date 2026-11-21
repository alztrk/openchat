import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const workflow = readFileSync(
  new URL('../../../../.github/workflows/desktop-ci.yml', import.meta.url),
  'utf8',
).replaceAll('\r\n', '\n');
const publishWorkflow = readFileSync(
  new URL('../../../../.github/workflows/publish-tested-release.yml', import.meta.url),
  'utf8',
).replaceAll('\r\n', '\n');
const releasePolicy = readFileSync(new URL('../../../../.github/RELEASE_POLICY.md', import.meta.url), 'utf8');

test('main and pull request package builds are not published or retained as release artifacts', () => {
  const releaseDraftJob = workflow.split('\n  release-draft:\n')[1]?.split(/\n {2}[a-z][a-z0-9-]*:\n/u)[0];
  const releaseJob = workflow.split('\n  release:\n')[1];
  assert.ok(releaseDraftJob);
  assert.ok(releaseJob);
  assert.match(releaseDraftJob, /if: startsWith\(github\.ref, 'refs\/tags\/v'\)/u);
  assert.match(releaseDraftJob, /gh release create .*--draft --verify-tag/u);
  assert.match(releaseJob, /gh release download "\$GITHUB_REF_NAME"/u);
  assert.match(releaseJob, /release-integrity\.mjs aggregate/u);
  assert.doesNotMatch(workflow, /--prerelease/u);
  assert.doesNotMatch(releaseJob, /gh release edit/u);

  for (const jobName of ['linux-packages', 'macos-packages', 'windows-installer']) {
    const job = workflow.split(`\n  ${jobName}:\n`)[1]?.split(/\n {2}[a-z][a-z0-9-]*:\n/u)[0];
    assert.ok(job, `${jobName} job should exist`);
    assert.match(job, /if: startsWith\(github\.ref, 'refs\/tags\/v'\)/u);
    assert.match(job, /needs:[\s\S]*?release-draft/u);
    assert.match(job, /permissions:\s+contents: write/u);
    assert.match(job, /gh release upload/u);
    assert.doesNotMatch(job, /actions\/upload-artifact/u);
  }
});

test('stable release publication is manual and requires passing QA on every target platform', () => {
  assert.match(publishWorkflow, /workflow_dispatch:/u);
  assert.match(publishWorkflow, /if: github\.ref == 'refs\/heads\/main'/u);

  for (const input of ['windows_nsis', 'linux_packages', 'macos_x64', 'macos_arm64']) {
    assert.match(publishWorkflow, new RegExp(`${input}:[\\s\\S]*?options:[\\s\\S]*?- not-tested[\\s\\S]*?- passed`, 'u'));
    assert.match(publishWorkflow, new RegExp(`\\$${input.toUpperCase()}_QA`, 'u'));
  }

  assert.match(publishWorkflow, /QA_EVIDENCE[\s\S]*?https:\/\//u);
  assert.match(publishWorkflow, /isDraft == true/u);
  assert.match(publishWorkflow, /endswith\("\.exe"\)/u);
  assert.match(publishWorkflow, /length >= 2/u);
  assert.match(publishWorkflow, /release-integrity\.mjs verify/u);
  assert.match(publishWorkflow, /gh release edit "\$RELEASE_TAG" --draft=false --verify-tag/u);
});

test('release policy requires clean-install and first-launch QA for NSIS and other packages', () => {
  assert.match(releasePolicy, /do not package, retain, or\s+publish installable preview artifacts/u);
  assert.match(releasePolicy, /must not create a public GitHub Release/u);
  assert.match(releasePolicy, /draft\*\* stable release/u);
  assert.match(releasePolicy, /Windows NSIS/u);
  assert.match(releasePolicy, /clean install/u);
  assert.match(releasePolicy, /local storage/u);
  assert.match(releasePolicy, /macOS Intel and Apple Silicon/u);
  assert.match(releasePolicy, /keep the\s+release as a draft/u);
});
