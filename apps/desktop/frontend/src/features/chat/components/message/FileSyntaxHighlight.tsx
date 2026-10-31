/**
 * Lazy-loaded syntax highlighting via TanStack Highlight.
 *
 * TanStack Highlight is a tiny (~5 KB gzip), synchronous syntax highlighter
 * designed for documentation and code snippets. It supports 79 languages
 * (including all of OpenChat's targets) and produces semantic CSS classes.
 *
 * This reduces the initial bundle by ~400 KB compared to Shiki since the
 * full language registry is not required at sync module evaluation.
 *
 * TanStack Highlight is imported dynamically after app mount (via
 * `preloadHighlighting`). Before it loads, plain text is shown. Once loaded,
 * components re-render with syntax highlighting.
 */
import type { ReactNode } from 'react';
import { useSyncExternalStore } from 'react';

import { detectFileLanguage } from './detectFileLanguage';

// ---------------------------------------------------------------------------
// Module-level state — shared across all component instances
// ---------------------------------------------------------------------------

const subscribers = new Set<() => void>();
let highlighterState: 'loading' | 'ready' | 'error' = 'loading';
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- highlighter types are complex
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
 * Lazy-load TanStack Highlight. Returns immediately if already loaded or failed.
 * Called once during app initialization.
 */
export function preloadHighlighting(): void {
  if (highlighterState !== 'loading') return;

  highlighterState = 'loading';

  // Import only the languages OpenChat actually uses
  Promise.all([
    import('@tanstack/highlight/core').then((m: any) => m.createHighlighter),
    import('@tanstack/highlight/languages/shell').then((m: any) => m.shell),
    import('@tanstack/highlight/languages/c').then((m: any) => m.c),
    import('@tanstack/highlight/languages/cpp').then((m: any) => m.cpp),
    import('@tanstack/highlight/languages/css').then((m: any) => m.css),
    import('@tanstack/highlight/languages/dockerfile').then((m: any) => m.dockerfile),
    import('@tanstack/highlight/languages/go').then((m: any) => m.go),
    import('@tanstack/highlight/languages/html').then((m: any) => m.html),
    import('@tanstack/highlight/languages/java').then((m: any) => m.java),
    import('@tanstack/highlight/languages/js').then((m: any) => m.js),
    import('@tanstack/highlight/languages/json').then((m: any) => m.json),
    import('@tanstack/highlight/languages/jsx').then((m: any) => m.jsx),
    import('@tanstack/highlight/languages/markdown').then((m: any) => m.markdown),
    import('@tanstack/highlight/languages/powershell').then((m: any) => m.powershell),
    import('@tanstack/highlight/languages/python').then((m: any) => m.python),
    import('@tanstack/highlight/languages/rust').then((m: any) => m.rust),
    import('@tanstack/highlight/languages/sql').then((m: any) => m.sql),
    import('@tanstack/highlight/languages/toml').then((m: any) => m.toml),
    import('@tanstack/highlight/languages/tsx').then((m: any) => m.tsx),
    import('@tanstack/highlight/languages/ts').then((m: any) => m.ts),
    import('@tanstack/highlight/languages/yaml').then((m: any) => m.yaml),
  ])
    .then(([createHighlighter, ...langs]) => {
      const hl = createHighlighter({ languages: langs });
      loadedHighlighter = hl;
      highlighterState = 'ready';
      notify();
    })
    .catch(() => {
      highlighterState = 'error';
      notify();
    });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export { detectFileLanguage };

/**
 * Highlight lines of code.
 * Returns an array where each element is a React node (array of spans) per line.
 * React renders nested arrays inline, so contents[index] works as expected.
 */
export function highlightFileLines(
  language: string | null,
  lines: string[],
  // biome-ignore lint/suspicious/noExplicitAny: typed as any for flexibility
): any {
  if (!language || !loadedHighlighter) return plainLines(lines);

  try {
    const code = lines.join('\n');
    const result = loadedHighlighter.highlight(code, { lang: language });
    const tokens = result?.tokens ?? [];

    // TanStack Highlight gives us flat tokens; we reconstruct line groups
    // by walking through the original code and splitting on newlines.
    let tokenIdx = 0;
    const lineGroups: ReactNode[][] = [];

    let currentLineEnd = code.indexOf('\n');

    for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
      const lineEnd = currentLineEnd === -1 ? code.length : currentLineEnd + 1;
      const lineTokens: Array<{ value: string; className?: string }> = [];
      let offsetInCode = 0;

      while (tokenIdx < tokens.length && offsetInCode < lineEnd) {
        const token = tokens[tokenIdx];
        if (offsetInCode >= lineEnd) break;
        if (offsetInCode + token.value.length <= lineEnd) {
          lineTokens.push(token);
          offsetInCode += token.value.length;
          tokenIdx += 1;
        } else {
          break;
        }
      }

      if (lineTokens.length === 0) {
        lineGroups.push(plainLines([lines[lineIdx] || ' ']));
      } else {
        lineGroups.push(
          lineTokens.map((t) => (
            <span key={t.value} className="shiki-token">
              {t.value}
            </span>
          )),
        );
      }

      currentLineEnd = code.indexOf('\n', lineEnd);
    }

    return lineGroups;
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

function plainLines(lines: string[]): ReactNode[][] {
  return lines.map((line) => [line]);
}
