/**
 * Lazy-loaded syntax highlighting via Shiki.
 *
 * Shiki is loaded asynchronously after app mount (via `preloadHighlighting`).
 * Before it loads, plain text is shown.  Once loaded, components re-render
 * with syntax highlighting.
 *
 * This reduces the initial bundle by ~2 MB since Shiki is not included
 * in the sync module evaluation path.
 */
import type { CSSProperties, ReactNode } from 'react';
import { useSyncExternalStore } from 'react';

import { detectFileLanguage } from './detectFileLanguage';

// ---------------------------------------------------------------------------
// Module-level state — shared across all component instances
// ---------------------------------------------------------------------------

const subscribers = new Set<() => void>();
let highlighterState: 'loading' | 'ready' | 'error' = 'loading';
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- shiki types are complex
let loadedHighlighter: any = null;

/**
 * Subscribe to highlighting state changes.
 * Returns an unsubscribe function.
 */
function subscribe(fn: () => void): () => void {
  subscribers.add(fn);
  return () => subscribers.delete(fn);
}

/** Notify all subscribers that highlighting state changed. */
function notify() {
  for (const fn of subscribers) fn();
}

/**
 * Lazy-load Shiki.  Returns immediately if already loaded or failed.
 * Called once during app initialization.
 */
export function preloadHighlighting(): void {
  if (highlighterState !== 'loading') return;

  highlighterState = 'loading';

  import('shiki/core')
    .then((coreMod) => {
      return import('shiki/engine/javascript').then((jsMod) => ({ core: coreMod, js: jsMod }));
    })
    .then(({ core: coreMod, js: jsMod }) => {
      // Import all language modules and extract their default exports
      const importModules = [
        () => import('shiki/langs/bash.mjs'),
        () => import('shiki/langs/c.mjs'),
        () => import('shiki/langs/cpp.mjs'),
        () => import('shiki/langs/css.mjs'),
        () => import('shiki/langs/docker.mjs'),
        () => import('shiki/langs/go.mjs'),
        () => import('shiki/langs/html.mjs'),
        () => import('shiki/langs/java.mjs'),
        () => import('shiki/langs/javascript.mjs'),
        () => import('shiki/langs/json.mjs'),
        () => import('shiki/langs/jsx.mjs'),
        () => import('shiki/langs/markdown.mjs'),
        () => import('shiki/langs/powershell.mjs'),
        () => import('shiki/langs/python.mjs'),
        () => import('shiki/langs/rust.mjs'),
        () => import('shiki/langs/sql.mjs'),
        () => import('shiki/langs/toml.mjs'),
        () => import('shiki/langs/tsx.mjs'),
        () => import('shiki/langs/typescript.mjs'),
        () => import('shiki/langs/yaml.mjs'),
        () => import('shiki/themes/github-dark.mjs'),
        () => import('shiki/themes/github-light.mjs'),
      ];

      return Promise.all(importModules.map((fn) => fn().then((m) => m.default))).then(
        ([
          bash, c, cpp, css, docker, go, html, java, javascript, json, jsx,
          markdown, powershell, python, rust, sql, toml, tsx, typescript, yaml,
          themeDark, themeLight,
        ]) => {
          // eslint-disable-next-line @typescript-eslint/no-unsafe-argument -- shiki types
          const { createHighlighterCoreSync } = coreMod;
          // eslint-disable-next-line @typescript-eslint/no-unsafe-argument -- shiki types
          const { createJavaScriptRegexEngine } = jsMod;

          // eslint-disable-next-line @typescript-eslint/no-explicit-any -- shiki types are complex
          const hl = createHighlighterCoreSync({
            engine: createJavaScriptRegexEngine(),
            themes: [themeDark, themeLight] as any,
            langs: [
              bash, c, cpp, css, docker, go, html, java, javascript, json, jsx,
              markdown, powershell, python, rust, sql, toml, tsx, typescript, yaml,
            ] as any,
          });

          loadedHighlighter = hl;
          highlighterState = 'ready';
          notify();
        },
      )
      .catch(() => {
        highlighterState = 'error';
        notify();
      });
    });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export { detectFileLanguage };

export function highlightFileLines(
  language: string | null,
  lines: string[],
  // biome-ignore lint/suspicious/noExplicitAny: typed as any for flexibility
): any {
  if (!language) return plainLines(lines);
  if (!KNOWN_LANGUAGES.has(language)) return plainLines(lines);
  if (highlighterState !== 'ready' || !loadedHighlighter) return plainLines(lines);

  try {
    const { tokens } = loadedHighlighter.codeToTokens(lines.join('\n'), {
      lang: language,
      themes: { light: 'github-light', dark: 'github-dark' },
      defaultColor: false,
    });
    return tokens.map((lineTokens: Array<{ offset: number; content: string; htmlStyle?: CSSProperties }>) =>
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
    return plainLines(lines);
  }
}

/**
 * React hook to subscribe to highlighting state changes.
 * Returns `'loading' | 'ready' | 'error'`.
 */
export function useHighlightingState(): 'loading' | 'ready' | 'error' {
  return useSyncExternalStore(subscribe, () => highlighterState, () => 'loading');
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

const KNOWN_LANGUAGES = new Set([
  'bash', 'c', 'cpp', 'css', 'docker', 'dockerfile', 'go',
  'html', 'java', 'javascript', 'json', 'jsx', 'markdown',
  'powershell', 'python', 'rust', 'sql', 'toml', 'tsx',
  'typescript', 'yaml',
]);

function plainLines(lines: string[]): ReactNode[][] {
  return lines.map((line) => [line]);
}
