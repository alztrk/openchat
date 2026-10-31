#!/usr/bin/env node
/**
 * Phase 1: Update all import paths from old locations to new shared/* locations.
 * Retry failed files.
 */
import { readFileSync, readdirSync, writeFileSync, accessSync, constants } from 'node:fs';
import { resolve } from 'node:path';

const SRC_ROOT = resolve(process.cwd(), 'apps/desktop/frontend/src');

function collectFiles(dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const full = resolve(dir, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(full));
    else if (entry.isFile() && (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx'))) files.push(full);
  }
  return files;
}

function applyReplacements(content) {
  let result = content;
  result = result.split('@/design-system"').join("@shared/components\"");
  result = result.split('@/design-system\'').join("@shared/components'");
  result = result.split('@/design-system/').join('@shared/components/');
  result = result.split('@/platform/local-service/').join('@shared/lib/platform/local-service/');
  result = result.split('@/platform/storage/').join('@shared/lib/platform/storage/');
  result = result.split('@/platform/tauri/').join('@shared/lib/platform/tauri/');
  result = result.split('@/platform/local-service\'').join("@shared/lib/platform/local-service'");
  result = result.split('@/platform/local-service"').join("@shared/lib/platform/local-service\"");
  result = result.split('@/platform/tauri"').join("@shared/lib/platform/tauri\"");
  result = result.split('@/platform/storage"').join("@shared/lib/platform/storage\"");
  result = result.split('@/platform/local-service ').join('@shared/lib/platform/local-service ');
  result = result.split('@/platform/tauri ').join('@shared/lib/platform/tauri ');
  result = result.split('@/platform/storage ').join('@shared/lib/platform/storage ');
  result = result.split('@/platform/LocalServiceClient').join('@shared/lib/platform/local-service/LocalServiceClient');
  result = result.split('@/platform/').join('@shared/lib/platform/');
  result = result.split('@/localization/i18n/').join('@shared/lib/i18n/');
  result = result.split('@/localization/i18n"').join('@shared/lib/i18n"');
  result = result.split('@/localization/locales/').join('@shared/lib/i18n/locales/');
  result = result.split('@/shared/utils/').join('@shared/lib/utils/');
  result = result.split('@/shared/utils"').join('@shared/lib/utils"');
  result = result.split('@/shared/utils\'').join("@shared/lib/utils'");
  result = result.split('@/shared/hooks/').join('@shared/lib/hooks/');
  result = result.split('@/shared/hooks"').join('@shared/lib/hooks"');
  result = result.split('@/shared/hooks\'').join("@shared/lib/hooks'");
  return result;
}

const files = collectFiles(SRC_ROOT);
let totalChanged = 0;
let totalSkipped = 0;
const skippedFiles = [];

for (const file of files) {
  try {
    accessSync(file, constants.W_OK);
  } catch {
    skippedFiles.push(file.replace(SRC_ROOT + '\\', ''));
    continue;
  }
  
  const content = readFileSync(file, 'utf-8');
  const needsUpdate = content.includes('@/design-system') ||
                      content.includes('@/platform/') ||
                      content.includes('@/localization/') ||
                      content.includes('@/shared/utils/') ||
                      content.includes('@/shared/hooks/');
  if (!needsUpdate) continue;
  
  try {
    const newContent = applyReplacements(content);
    if (newContent !== content) {
      writeFileSync(file, newContent, 'utf-8');
      totalChanged++;
    }
  } catch (e) {
    skippedFiles.push(file.replace(SRC_ROOT + '\\', ''));
    totalSkipped++;
  }
}

console.log(`Phase 1: Updated ${totalChanged} files, skipped ${skippedFiles.length}\n`);
for (const f of skippedFiles) console.log(`  ⚠️  ${f}`);
