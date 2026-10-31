#!/usr/bin/env node
/**
 * Fix all crate:: imports in domain files.
 * After moving domain files to the domain crate, crate::X should become crate::X (since X is a top-level module).
 * But some files have incorrect #[path] references that need fixing.
 */
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const SRC_ROOT = resolve(process.cwd(), 'crates/openchat-domain/src');

function collectFiles(dir) {
  const entries = readdirSync(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const full = resolve(dir, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(full));
    else if (entry.isFile() && entry.name.endsWith('.rs')) files.push(full);
  }
  return files;
}

function applyFix(content) {
  let result = content;
  
  // Fix: crate::storage:: → crate::storage:: (already correct for top-level module)
  // Fix: crate::protocol:: → crate::protocol:: (already correct)
  // Fix: crate::sandbox:: → crate::sandbox:: (already correct)
  // Fix: crate::secrets:: → crate::secrets:: (already correct)
  // Fix: crate::rpc:: → crate::rpc:: (already correct)
  // Fix: crate::transport:: → crate::transport:: (already correct)
  // Fix: crate::user_question_broker:: → crate::user_question_broker:: (already correct)
  // Fix: crate::conversation_limits:: → crate::conversation_limits:: (already correct)
  // Fix: crate::provider_ids:: → crate::provider_ids:: (already correct)
  // Fix: crate::storage_broker_test_support:: → crate::storage_broker_test_support:: (already correct)
  
  // Fix: #[path = "settings/instructions.rs"] needs to match actual path
  // Fix: missing module declarations
  
  return result;
}

const files = collectFiles(SRC_ROOT);
let totalChanged = 0;

for (const file of files) {
  const content = readFileSync(file, 'utf-8');
  const newContent = applyFix(content);
  if (newContent !== content) {
    writeFileSync(file, newContent, 'utf-8');
    totalChanged++;
  }
}

console.log(`Fixed ${totalChanged} files`);
