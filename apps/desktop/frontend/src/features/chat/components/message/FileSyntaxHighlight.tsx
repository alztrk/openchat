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
 * TanStack Highlight is loaded when a view containing code first mounts.
 * Before it loads, plain text is shown. Once loaded, components re-render
 * with syntax highlighting.
 */
import type { ReactNode } from 'react';
import { useEffect, useSyncExternalStore } from 'react';
import type { Highlighter, HighlightToken } from '@tanstack/highlight/core';

import { detectFileLanguage } from './detectFileLanguage';

// ---------------------------------------------------------------------------
// Module-level state — shared across all component instances
// ---------------------------------------------------------------------------

const subscribers = new Set<() => void>();
let highlighterState: 'loading' | 'ready' | 'error' = 'loading';
let loadedHighlighter: Highlighter | null = null;
let highlighterPromise: Promise<void> | undefined;

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
 * Called when a code or file view first needs highlighting.
 */
function preloadHighlighting(): void {
  if (highlighterState !== 'loading' || highlighterPromise) return;

  highlighterPromise = import('./fileHighlightingRuntime')
    .then(({ loadFileHighlighter }) => loadFileHighlighter())
    .then(
      (highlighter) => {
        loadedHighlighter = highlighter;
        highlighterState = 'ready';
      },
      () => {
        highlighterState = 'error';
      },
    )
    .then(notify);
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
): ReactNode[][] {
  if (!language || !loadedHighlighter) return plainLines(lines);

  try {
    const code = lines.join('\n');
    const result = loadedHighlighter.highlight(code, { lang: language });
    const lineGroups = lines.map((): ReactNode[] => []);
    const keyOccurrences = new Map<string, number>();
    let lineIndex = 0;

    result.tokens.forEach((token: HighlightToken) => {
      token.value.split('\n').forEach((piece, pieceIndex, pieces) => {
        const currentLine = lineGroups[lineIndex];
        if (piece.length > 0 && currentLine) {
          const className = token.className ? `th-token th-${token.className}` : 'th-token';
          const tokenKey = `${lineIndex}:${token.className ?? 'plain'}:${piece}`;
          const occurrence = keyOccurrences.get(tokenKey) ?? 0;
          keyOccurrences.set(tokenKey, occurrence + 1);
          currentLine.push(
            <span className={className} key={`${tokenKey}:${occurrence}`}>
              {piece}
            </span>,
          );
        }
        if (pieceIndex < pieces.length - 1) lineIndex += 1;
      });
    });

    return lineGroups.map((line, index) => (line.length > 0 ? line : [lines[index] || ' ']));
  } catch {
    return plainLines(lines);
  }
}

/**
 * React hook to subscribe to highlighting state changes.
 * Returns `'loading' | 'ready' | 'error'`.
 */
export function useHighlightingState(): 'loading' | 'ready' | 'error' {
  useEffect(preloadHighlighting, []);
  return useSyncExternalStore(subscribe, () => highlighterState, () => 'loading');
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function plainLines(lines: string[]): ReactNode[][] {
  return lines.map((line) => [line]);
}
