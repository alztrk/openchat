const stableVersionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/u;
const commitPattern = /^[0-9a-f]{7,40}$/u;

export function parseBuildChannel(args) {
  if (args.length !== 1 || !['--preview', '--stable'].includes(args[0])) {
    throw new Error('Choose exactly one release channel: --preview or --stable.');
  }

  return args[0].slice(2);
}

export function assertCleanReleaseTree(status) {
  if (status !== '') {
    throw new Error('Release packages must be built from a clean Git working tree. Commit the intended changes first.');
  }
}

export function resolveReleaseIdentity({ channel, baseVersion, commitCount, commitSha, tag }) {
  if (!stableVersionPattern.test(baseVersion)) {
    throw new Error('The configured application version must use MAJOR.MINOR.PATCH format.');
  }
  if (!Number.isSafeInteger(commitCount) || commitCount < 1 || !commitPattern.test(commitSha)) {
    throw new Error('Git did not provide a valid commit number and commit SHA.');
  }

  if (channel === 'stable') {
    if (tag !== `v${baseVersion}`) {
      throw new Error(`A stable build requires the exact Git tag v${baseVersion} at HEAD.`);
    }
    return {
      channel,
      version: `${baseVersion}+${commitCount}.g${commitSha}`,
      buildId: `stable.${commitCount}.g${commitSha}`,
    };
  }

  if (channel !== 'preview') {
    throw new Error('The release channel must be preview or stable.');
  }

  return {
    channel,
    version: `${baseVersion}-preview.${commitCount}.g${commitSha}`,
    buildId: `preview.${commitCount}.g${commitSha}`,
  };
}
