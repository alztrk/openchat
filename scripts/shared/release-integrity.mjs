import { createHash } from 'node:crypto';
import { createReadStream, lstatSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const checksumFileName = 'SHA256SUMS';

function isSafeArtifactName(name) {
  return name !== '.' && name !== '..' && !/[\r\n\\/\u0000-\u001f]/u.test(name);
}

function listPayloadFiles(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.name !== checksumFileName)
    .map((entry) => {
      if (!entry.isFile() || !isSafeArtifactName(entry.name)) {
        throw new Error(`Release artifact directory contains an unsupported entry: ${entry.name}`);
      }
      return entry.name;
    })
    .sort((left, right) => left.localeCompare(right, 'en'));
}

async function sha256(filePath) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest('hex');
}

export async function createReleaseManifest(directory) {
  const files = listPayloadFiles(directory);
  if (files.length === 0) throw new Error('Release artifact directory contains no package files.');

  const entries = await Promise.all(files.map(async (fileName) => `${await sha256(path.join(directory, fileName))}  ${fileName}`));
  const manifest = `${entries.join('\n')}\n`;
  writeFileSync(path.join(directory, checksumFileName), manifest, { encoding: 'utf8', flag: 'wx' });
  return files.length;
}

export async function verifyReleaseManifest(directory) {
  const manifestPath = path.join(directory, checksumFileName);
  if (!lstatSync(manifestPath).isFile()) throw new Error('SHA256SUMS must be a regular file.');
  const manifest = readFileSync(manifestPath, 'utf8');
  const entries = manifest.split('\n').filter(Boolean).map((line) => {
    const match = /^([0-9a-f]{64})  ([^\r\n\\/\u0000-\u001f]+)$/u.exec(line);
    if (!match || !isSafeArtifactName(match[2])) throw new Error('SHA256SUMS contains a malformed entry.');
    return { expectedHash: match[1], fileName: match[2] };
  });
  if (entries.length === 0) throw new Error('SHA256SUMS contains no package entries.');

  const names = entries.map((entry) => entry.fileName);
  if (new Set(names).size !== names.length) throw new Error('SHA256SUMS contains duplicate package entries.');

  const actualNames = listPayloadFiles(directory);
  const expectedNames = [...names].sort((left, right) => left.localeCompare(right, 'en'));
  if (actualNames.length !== names.length || actualNames.some((name, index) => name !== expectedNames[index])) {
    throw new Error('SHA256SUMS does not cover exactly the package files in the artifact.');
  }

  for (const { expectedHash, fileName } of entries) {
    if ((await sha256(path.join(directory, fileName))) !== expectedHash) {
      throw new Error(`Release artifact integrity check failed for ${fileName}.`);
    }
  }

  return entries.length;
}

export async function verifyReleaseFiles(directory, fileNames) {
  const names = [...(fileNames ?? listPayloadFiles(directory))].sort((left, right) => left.localeCompare(right, 'en'));
  if (names.length === 0) throw new Error('Cannot create a release checksum manifest without files.');
  if (new Set(names).size !== names.length) throw new Error('Release files contain duplicate names.');
  for (const name of names) {
    if (path.basename(name) !== name || !isSafeArtifactName(name) || !lstatSync(path.join(directory, name)).isFile()) {
      throw new Error(`Release file is not a regular file in the release directory: ${name}`);
    }
  }
  const entries = await Promise.all(names.map(async (name) => `${await sha256(path.join(directory, name))}  ${name}`));
  return `${entries.join('\n')}\n`;
}
