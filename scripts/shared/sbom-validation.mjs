import { readFileSync } from 'node:fs';

export function validateSpdxJson(jsonText) {
  let document;
  try {
    document = JSON.parse(jsonText);
  } catch {
    throw new Error('SBOM must contain valid JSON.');
  }

  if (typeof document !== 'object' || document === null || Array.isArray(document)) {
    throw new Error('SBOM document must be a JSON object.');
  }
  if (document.spdxVersion !== 'SPDX-2.3') {
    throw new Error('SBOM spdxVersion must be SPDX-2.3.');
  }
  if (!Array.isArray(document.packages) || document.packages.length === 0) {
    throw new Error('SBOM packages must be a non-empty array.');
  }
  if (typeof document.documentNamespace !== 'string' || document.documentNamespace.trim().length === 0) {
    throw new Error('SBOM documentNamespace must be non-empty.');
  }

  return { packageCount: document.packages.length };
}

export function validateSpdxFile(filePath) {
  let jsonText;
  try {
    jsonText = readFileSync(filePath, 'utf8');
  } catch {
    throw new Error('SBOM file is missing or unreadable.');
  }
  return validateSpdxJson(jsonText);
}
