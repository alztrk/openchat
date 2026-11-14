import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { createReleaseManifest, verifyReleaseManifest, verifyReleaseFiles } from './shared/release-integrity.mjs';

const [command, directory, ...arguments_] = process.argv.slice(2);

try {
  if (command === 'create' && directory && arguments_.length === 0) {
    const count = await createReleaseManifest(directory);
    process.stdout.write(`Created SHA256SUMS for ${count} release packages.\n`);
  } else if (command === 'verify' && directory && arguments_.length === 0) {
    const count = await verifyReleaseManifest(directory);
    process.stdout.write(`Verified SHA256SUMS for ${count} release packages.\n`);
  } else if (command === 'aggregate' && directory && arguments_.length === 0) {
    const manifest = await verifyReleaseFiles(directory);
    writeFileSync(path.join(directory, 'SHA256SUMS'), manifest, { encoding: 'utf8', flag: 'wx' });
    process.stdout.write(`Created release SHA256SUMS for ${manifest.trimEnd().split('\n').length} published files.\n`);
  } else {
    throw new Error('Usage: node scripts/release-integrity.mjs <create|verify|aggregate> <directory>');
  }
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : 'Release integrity operation failed.'}\n`);
  process.exitCode = 1;
}
