#!/usr/bin/env node
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const SRC = resolve(process.cwd(), 'crates/openchat-domain/src');

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

const files = collectFiles(SRC);
let changed = 0;

for (const file of files) {
  let content = readFileSync(file, 'utf-8');
  const before = content;
  
  // Fix: crate::domain:: → crate::
  content = content.replace(/crate::domain::/g, 'crate::');
  
  // Fix: use crate::domain:: → use crate::
  content = content.replace(/use crate::domain::/g, 'use crate::');
  
  if (content !== before) {
    writeFileSync(file, content, 'utf-8');
    changed++;
  }
}

console.log(`Fixed ${changed} files`);
