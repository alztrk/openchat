import { createHighlighter } from '@tanstack/highlight/core';

export async function loadFileHighlighter() {
  const languages = await Promise.all([
    import('@tanstack/highlight/languages/shell').then((module) => module.shell),
    import('@tanstack/highlight/languages/c').then((module) => module.c),
    import('@tanstack/highlight/languages/cpp').then((module) => module.cpp),
    import('@tanstack/highlight/languages/css').then((module) => module.css),
    import('@tanstack/highlight/languages/dockerfile').then((module) => module.dockerfile),
    import('@tanstack/highlight/languages/go').then((module) => module.go),
    import('@tanstack/highlight/languages/html').then((module) => module.html),
    import('@tanstack/highlight/languages/java').then((module) => module.java),
    import('@tanstack/highlight/languages/js').then((module) => module.js),
    import('@tanstack/highlight/languages/json').then((module) => module.json),
    import('@tanstack/highlight/languages/jsx').then((module) => module.jsx),
    import('@tanstack/highlight/languages/markdown').then((module) => module.markdown),
    import('@tanstack/highlight/languages/powershell').then((module) => module.powershell),
    import('@tanstack/highlight/languages/python').then((module) => module.python),
    import('@tanstack/highlight/languages/rust').then((module) => module.rust),
    import('@tanstack/highlight/languages/sql').then((module) => module.sql),
    import('@tanstack/highlight/languages/toml').then((module) => module.toml),
    import('@tanstack/highlight/languages/tsx').then((module) => module.tsx),
    import('@tanstack/highlight/languages/ts').then((module) => module.ts),
    import('@tanstack/highlight/languages/yaml').then((module) => module.yaml),
  ]);

  return createHighlighter({ languages });
}
