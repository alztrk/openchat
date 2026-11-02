const fileLanguages: Readonly<Record<string, string>> = {
  bash: 'shell',
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
  js: 'js',
  jsx: 'jsx',
  json: 'json',
  jsonc: 'json',
  md: 'markdown',
  markdown: 'markdown',
  ps1: 'powershell',
  py: 'python',
  rs: 'rust',
  sh: 'shell',
  sql: 'sql',
  toml: 'toml',
  ts: 'ts',
  tsx: 'tsx',
  xml: 'html',
  yaml: 'yaml',
  yml: 'yaml',
};

export function detectFileLanguage(path: string): string | null {
  const filename = path.replaceAll('\\', '/').split('/').at(-1)?.toLowerCase() ?? '';
  if (filename === 'dockerfile' || filename.startsWith('dockerfile.')) return 'dockerfile';
  const extension = filename.split('.').at(-1);
  return extension ? (fileLanguages[extension] ?? null) : null;
}
