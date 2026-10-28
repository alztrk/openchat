import type { CSSProperties, ReactNode } from 'react';
import { createHighlighterCoreSync } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';

import bash from 'shiki/langs/bash.mjs';
import c from 'shiki/langs/c.mjs';
import cpp from 'shiki/langs/cpp.mjs';
import css from 'shiki/langs/css.mjs';
import docker from 'shiki/langs/docker.mjs';
import go from 'shiki/langs/go.mjs';
import html from 'shiki/langs/html.mjs';
import java from 'shiki/langs/java.mjs';
import javascript from 'shiki/langs/javascript.mjs';
import json from 'shiki/langs/json.mjs';
import jsx from 'shiki/langs/jsx.mjs';
import markdown from 'shiki/langs/markdown.mjs';
import powershell from 'shiki/langs/powershell.mjs';
import python from 'shiki/langs/python.mjs';
import rust from 'shiki/langs/rust.mjs';
import sql from 'shiki/langs/sql.mjs';
import toml from 'shiki/langs/toml.mjs';
import tsx from 'shiki/langs/tsx.mjs';
import typescript from 'shiki/langs/typescript.mjs';
import yaml from 'shiki/langs/yaml.mjs';

import themeDark from 'shiki/themes/github-dark.mjs';
import themeLight from 'shiki/themes/github-light.mjs';

const highlighter = createHighlighterCoreSync({
  engine: createJavaScriptRegexEngine(),
  themes: [themeDark, themeLight],
  langs: [
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
  ],
});

const registeredLanguages = new Set([
  'bash',
  'c',
  'cpp',
  'css',
  'docker',
  'dockerfile',
  'go',
  'html',
  'java',
  'javascript',
  'json',
  'jsx',
  'markdown',
  'powershell',
  'python',
  'rust',
  'sql',
  'toml',
  'tsx',
  'typescript',
  'yaml',
]);

const fileLanguages: Readonly<Record<string, string>> = {
  bash: 'bash',
  c: 'c',
  cc: 'cpp',
  cpp: 'cpp',
  css: 'css',
  go: 'go',
  h: 'c',
  hpp: 'cpp',
  html: 'html',
  htm: 'html',
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
  xml: 'html',
  yaml: 'yaml',
  yml: 'yaml',
};

export function detectFileLanguage(path: string): string | null {
  const filename = path.replaceAll('\\', '/').split('/').at(-1)?.toLowerCase() ?? '';
  if (filename === 'dockerfile' || filename.startsWith('dockerfile.')) return 'docker';
  const extension = filename.split('.').at(-1);
  return extension ? (fileLanguages[extension] ?? null) : null;
}

export function highlightFileLines(language: string | null, lines: string[]): ReactNode[][] {
  if (!language || !registeredLanguages.has(language)) return lines.map((line) => [line]);

  try {
    const { tokens } = highlighter.codeToTokens(lines.join('\n'), {
      lang: language,
      themes: { light: 'github-light', dark: 'github-dark' },
      defaultColor: false,
    });

    return tokens.map((lineTokens) =>
      lineTokens.map((token) => {
        const style: CSSProperties = { ...token.htmlStyle };
        return (
          <span className="shiki-token" key={`${token.offset}-${token.content}`} style={style}>
            {token.content}
          </span>
        );
      }),
    );
  } catch {
    return lines.map((line) => [line]);
  }
}
