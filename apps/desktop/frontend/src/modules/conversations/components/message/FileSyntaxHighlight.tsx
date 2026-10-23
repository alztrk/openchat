import type { Element, RootContent } from 'hast';
import type { ReactNode } from 'react';
import bash from 'refractor/bash';
import c from 'refractor/c';
import { refractor } from 'refractor/core';
import cpp from 'refractor/cpp';
import css from 'refractor/css';
import docker from 'refractor/docker';
import go from 'refractor/go';
import java from 'refractor/java';
import javascript from 'refractor/javascript';
import json from 'refractor/json';
import jsx from 'refractor/jsx';
import markdown from 'refractor/markdown';
import html from 'refractor/markup';
import powershell from 'refractor/powershell';
import python from 'refractor/python';
import rust from 'refractor/rust';
import sql from 'refractor/sql';
import toml from 'refractor/toml';
import tsx from 'refractor/tsx';
import typescript from 'refractor/typescript';
import yaml from 'refractor/yaml';

const syntaxes = [
  bash,
  c,
  cpp,
  css,
  docker,
  go,
  html,
  java,
  javascript,
  json,
  jsx,
  markdown,
  powershell,
  python,
  rust,
  sql,
  toml,
  tsx,
  typescript,
  yaml,
];

for (const syntax of syntaxes) refractor.register(syntax);

const fileLanguages: Readonly<Record<string, string>> = {
  bash: 'bash',
  c: 'c',
  cc: 'cpp',
  cpp: 'cpp',
  css: 'css',
  go: 'go',
  h: 'c',
  hpp: 'cpp',
  html: 'markup',
  htm: 'markup',
  java: 'java',
  js: 'javascript',
  jsx: 'jsx',
  json: 'json',
  jsonc: 'json',
  md: 'markdown',
  markdown: 'markdown',
  ps1: 'powershell',
  py: 'python',
  rs: 'rust',
  sh: 'bash',
  sql: 'sql',
  toml: 'toml',
  ts: 'typescript',
  tsx: 'tsx',
  xml: 'markup',
  yaml: 'yaml',
  yml: 'yaml',
};

export function detectFileLanguage(path: string): string | null {
  const filename = path.replaceAll('\\', '/').split('/').at(-1)?.toLowerCase() ?? '';
  if (filename === 'dockerfile' || filename.startsWith('dockerfile.')) return 'docker';
  const extension = filename.split('.').at(-1);
  return extension ? (fileLanguages[extension] ?? null) : null;
}

interface CodeRun {
  classNames: string[];
  offset: number;
  text: string;
}

export function highlightFileLines(language: string | null, lines: string[]): ReactNode[][] {
  if (!language || !refractor.registered(language)) return lines.map((line) => [line]);

  const highlighted = refractor.highlight(lines.join('\n'), language);
  const runsByLine: CodeRun[][] = [[]];
  appendNodes(highlighted.children, [], runsByLine, { offset: 0 });
  return runsByLine.map((runs) =>
    runs.map((run) => (
      <span
        className={run.classNames.map((name) => `tool-code-token-${name}`).join(' ')}
        key={`${run.offset}-${run.classNames.join('.')}`}
      >
        {run.text}
      </span>
    )),
  );
}

function appendNodes(
  nodes: RootContent[],
  parentClasses: string[],
  runsByLine: CodeRun[][],
  cursor: { offset: number },
): void {
  for (const node of nodes) {
    if (node.type === 'text') {
      appendText(node.value, parentClasses, runsByLine, cursor);
      continue;
    }
    if (node.type !== 'element') continue;
    appendNodes(node.children, tokenClasses(node, parentClasses), runsByLine, cursor);
  }
}

function tokenClasses(node: Element, parentClasses: string[]): string[] {
  if (node.tagName !== 'span') return parentClasses;
  const classes = node.properties.className;
  const classNames = classes ?? [];
  const safeNames = classNames.filter((name): name is string => typeof name === 'string' && /^[a-z-]+$/.test(name));
  return [...parentClasses, ...safeNames];
}

function appendText(value: string, classNames: string[], runsByLine: CodeRun[][], cursor: { offset: number }): void {
  const segments = value.split('\n');
  segments.forEach((text, index) => {
    if (text.length > 0) runsByLine.at(-1)?.push({ classNames, offset: cursor.offset, text });
    cursor.offset += text.length;
    if (index < segments.length - 1) {
      cursor.offset += 1;
      runsByLine.push([]);
    }
  });
}
