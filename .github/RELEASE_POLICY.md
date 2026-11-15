# Release policy

OpenChat releases stay unpublished until the exact packaged artifacts pass CI and
manual install and first-launch checks. A successful source build, unit test run,
signature check, or checksum check alone is not package QA.

## Candidate builds

- Pull requests and pushes to `main` run validation but do not package, retain, or
  publish installable preview artifacts.
- They must not create a public GitHub Release or a publicly visible preview
  release.
- A `vMAJOR.MINOR.PATCH` tag may create a **draft** stable release only after all
  platform validation, package, signing, and SBOM jobs pass.
- Draft release assets are candidates for testing. Do not share their download
  links outside the release team.

## Required QA before publication

Run the `Publish tested stable release` workflow from `main` for the exact draft
tag. All platform results must be `passed`, and the workflow must verify the
draft's complete artifact set and `SHA256SUMS` before publishing it.

For each platform, install the exact draft artifact in a clean supported
environment and verify that OpenChat starts, initializes local storage, opens a
new chat, and can be closed and reopened without a startup or storage error.

- **Windows NSIS:** run the `.exe` installer, complete a clean install, launch
  OpenChat, confirm there is no storage-broker or startup error, and reopen the
  installed application.
- **Linux:** install and launch the `.deb` and AppImage packages in a clean
  supported environment, then reopen the application.
- **macOS Intel and Apple Silicon:** install and launch each architecture's DMG
  on its matching hardware or supported runner, then reopen the application.

Record a link to the QA evidence for the exact tag in the publish workflow input.
Never mark a platform passed based only on a different platform, a development
  build, or a source-level test. If any check fails or remains untested, keep the
  release as a draft and fix or retest it before publication.
