import { validateSpdxFile } from './shared/sbom-validation.mjs';

const [filePath, ...extraArguments] = process.argv.slice(2);
if (!filePath || extraArguments.length > 0) {
  process.stderr.write('Usage: node scripts/validate-sbom.mjs <path-to-spdx-json>\n');
  process.exitCode = 2;
} else {
  try {
    const result = validateSpdxFile(filePath);
    process.stdout.write(`Validated ${filePath}: ${result.packageCount} packages.\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : 'SBOM validation failed.'}\n`);
    process.exitCode = 1;
  }
}
