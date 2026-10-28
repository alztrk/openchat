import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const localeNames = ['de', 'en', 'es', 'fr', 'tr'];
const localeDirectory = new URL('../src/localization/locales/', import.meta.url);
const locales = new Map(
  localeNames.map((locale) => {
    const file = fileURLToPath(new URL(`${locale}.json`, localeDirectory));
    return [locale, JSON.parse(readFileSync(file, 'utf8'))];
  }),
);
const reference = locales.get('en');
const referenceKeys = new Set(Object.keys(reference));
const failures = [];

for (const [locale, messages] of locales) {
  for (const key of referenceKeys) {
    if (!(key in messages)) {
      failures.push(`${locale}: missing translation key "${key}"`);
      continue;
    }
    const expected = placeholders(reference[key]);
    const actual = placeholders(messages[key]);
    if (expected.join('\0') !== actual.join('\0')) {
      failures.push(`${locale}: placeholder mismatch for "${key}" (expected ${expected.join(', ') || 'none'}; found ${actual.join(', ') || 'none'})`);
    }
  }
  for (const key of Object.keys(messages)) {
    if (!referenceKeys.has(key)) failures.push(`${locale}: translation key "${key}" is missing from en.json`);
  }
}

if (failures.length > 0) {
  process.stderr.write(`${failures.join('\n')}\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(`Locale keys and placeholders match across ${localeNames.join(', ')}.\n`);
}

function placeholders(value) {
  if (typeof value !== 'string') return [];
  const found = [];
  let depth = 0;
  for (let index = 0; index < value.length; index += 1) {
    if (value.startsWith('{{', index)) {
      const end = value.indexOf('}}', index + 2);
      if (end !== -1) {
        const name = value.slice(index + 2, end).trim();
        if (/^[\w.-]+$/.test(name)) found.push(name);
        index = end + 1;
      }
      continue;
    }
    const character = value[index];
    if (character === '{') {
      if (depth === 0) {
        const separator = value.indexOf(',', index + 1);
        const closing = value.indexOf('}', index + 1);
        const end = separator === -1 ? closing : closing === -1 ? separator : Math.min(separator, closing);
        if (end !== -1) {
          const name = value.slice(index + 1, end).trim();
          if (/^[\w.-]+$/.test(name)) found.push(name);
        }
      }
      depth += 1;
    } else if (character === '}' && depth > 0) {
      depth -= 1;
    }
  }
  return found.sort();
}
