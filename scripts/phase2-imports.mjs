#!/usr/bin/env node
/**
 * Phase 2: Update all import paths for conversations → chat feature migration.
 */
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
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
  
  // conversations components
  result = result.split('@/modules/conversations/components/').join('@features/chat/components/');
  result = result.split('@/modules/conversations/components"').join("@features/chat/components\"");
  result = result.split('@/modules/conversations/components\'').join("@features/chat/components'");
  result = result.split('@/modules/conversations/components ').join('@features/chat/components ');
  
  // conversations data (but NOT data/chatX which goes to state)
  result = result.split('@/modules/conversations/data/').join('@features/chat/data/');
  result = result.split('@/modules/conversations/data"').join("@features/chat/data\"");
  result = result.split('@/modules/conversations/data\'').join("@features/chat/data'");
  result = result.split('@/modules/conversations/data ').join('@features/chat/data ');
  
  // conversations stores → state
  result = result.split('@/modules/conversations/stores/').join('@features/chat/state/');
  result = result.split('@/modules/conversations/stores"').join("@features/chat/state\"");
  result = result.split('@/modules/conversations/stores\'').join("@features/chat/state'");
  result = result.split('@/modules/conversations/stores ').join('@features/chat/state ');
  
  // conversations hooks
  result = result.split('@/modules/conversations/hooks/').join('@features/chat/hooks/');
  result = result.split('@/modules/conversations/hooks"').join("@features/chat/hooks\"");
  result = result.split('@/modules/conversations/hooks\'').join("@features/chat/hooks'");
  result = result.split('@/modules/conversations/hooks ').join('@features/chat/hooks ');
  
  // conversations message/ → components/message/
  result = result.split('@/modules/conversations/message/').join('@features/chat/components/message/');
  
  // Other modules → features
  result = result.split('@/modules/settings/').join('@features/app-settings/');
  result = result.split('@/modules/providers/').join('@features/provider-settings/');
  result = result.split('@/modules/design/').join('@features/design-workspace/');
  result = result.split('@/modules/model-library/').join('@features/model-catalog/');
  result = result.split('@/modules/projects/').join('@features/projects/');
  result = result.split('@/modules/notifications/').join('@features/notifications/');
  result = result.split('@/modules/security-scans/').join('@features/security-scans/');
  
  return result;
}

const files = collectFiles(SRC_ROOT);
let totalChanged = 0;
const changedFiles = [];

for (const file of files) {
  try {
    const content = readFileSync(file, 'utf-8');
    const needsUpdate = content.includes('@/modules/conversations') ||
                        content.includes('@/modules/settings') ||
                        content.includes('@/modules/providers') ||
                        content.includes('@/modules/design/') ||
                        content.includes('@/modules/model-library') ||
                        content.includes('@/modules/projects') ||
                        content.includes('@/modules/notifications') ||
                        content.includes('@/modules/security-scans');
    if (!needsUpdate) continue;
    
    const newContent = applyReplacements(content);
    if (newContent !== content) {
      writeFileSync(file, newContent, 'utf-8');
      totalChanged++;
      changedFiles.push(file.replace(SRC_ROOT + '\\', ''));
    }
  } catch (e) {
    // skip unreadable files
  }
}

console.log(`Phase 2: Updated ${totalChanged} files\n`);
for (const f of changedFiles) console.log(`  ${f}`);
