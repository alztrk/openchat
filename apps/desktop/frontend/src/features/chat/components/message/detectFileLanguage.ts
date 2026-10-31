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
