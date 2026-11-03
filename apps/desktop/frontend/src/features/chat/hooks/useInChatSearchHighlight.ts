import { type RefObject, useEffect, useRef } from 'react';

import { normalizeSearchText } from '@app/runtime/chatStorage';
import { useChatStore } from '@app/runtime/chatStore';

const HIGHLIGHT_STYLE_ID = 'openchat-search-highlight-style';

function ensureHighlightStyle(): void {
  if (typeof document === 'undefined') return;
  if (document.getElementById(HIGHLIGHT_STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = HIGHLIGHT_STYLE_ID;
  style.textContent = `
    ::highlight(openchat-search-active) {
      background-color: #facc15 !important;
      color: #000000 !important;
    }
    ::highlight(openchat-search-match) {
      background-color: rgba(250, 204, 21, 0.45) !important;
      color: inherit !important;
    }
  `;
  document.head.appendChild(style);
}

/**
 * Highlights matching terms across rendered message text nodes in the conversation viewport
 * using the CSS Custom Highlight API without mutating DOM text or disrupting React's virtual DOM.
 */
export function useInChatSearchHighlight(
  containerRef: RefObject<HTMLElement | null>,
  query: string,
  activeTargetId: string | null,
  renderRevision: unknown,
): void {
  const searchMatchIndex = useChatStore((state) => state.searchMatchIndex);
  const setSearchMatchInfo = useChatStore((state) => state.setSearchMatchInfo);
  const rangesRef = useRef<Range[]>([]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: renderRevision intentionally retriggers the scan after message renders.
  useEffect(() => {
    if (typeof CSS === 'undefined' || !('highlights' in CSS)) return;
    ensureHighlightStyle();

    const normalizedQuery = normalizeSearchText(query.trim());
    if (!normalizedQuery) {
      CSS.highlights.delete('openchat-search-match');
      CSS.highlights.delete('openchat-search-active');
      rangesRef.current = [];
      setSearchMatchInfo(0, 0);
      return;
    }

    const scan = () => {
      const container = containerRef.current;
      if (!container) return;

      const discoveredRanges: Range[] = [];
      let targetInitialIndex = -1;

      const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
        acceptNode(node) {
          if (!node.nodeValue?.trim()) return NodeFilter.FILTER_SKIP;
          const parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_SKIP;
          const tag = parent.tagName.toLowerCase();
          if (tag === 'input' || tag === 'textarea' || tag === 'button') {
            return NodeFilter.FILTER_REJECT;
          }
          return NodeFilter.FILTER_ACCEPT;
        },
      });

      let currentNode = walker.nextNode();
      while (currentNode !== null) {
        const text = currentNode.nodeValue ?? '';
        const normalizedText = normalizeSearchText(text);
        let startIndex = 0;

        while (startIndex < normalizedText.length) {
          const foundIndex = normalizedText.indexOf(normalizedQuery, startIndex);
          if (foundIndex === -1) break;

          try {
            const range = new Range();
            range.setStart(currentNode, foundIndex);
            range.setEnd(currentNode, foundIndex + normalizedQuery.length);

            const messageArticle = currentNode.parentElement?.closest('[id^="openchat-message-"]');
            if (
              targetInitialIndex === -1 &&
              activeTargetId &&
              messageArticle?.id === `openchat-message-${activeTargetId}`
            ) {
              targetInitialIndex = discoveredRanges.length;
            }

            discoveredRanges.push(range);
          } catch {
            // Boundary adjustments during dynamic render can be safely ignored
          }

          startIndex = foundIndex + Math.max(1, normalizedQuery.length);
        }

        currentNode = walker.nextNode();
      }

      rangesRef.current = discoveredRanges;

      if (discoveredRanges.length > 0) {
        const currentMatchIndex = useChatStore.getState().searchMatchIndex;
        const activeIdx =
          targetInitialIndex !== -1 && currentMatchIndex === 0
            ? targetInitialIndex
            : Math.min(currentMatchIndex, discoveredRanges.length - 1);

        setSearchMatchInfo(activeIdx, discoveredRanges.length);

        const activeRange = discoveredRanges[activeIdx];
        const matchRanges = discoveredRanges.filter((_, i) => i !== activeIdx);

        try {
          CSS.highlights.set('openchat-search-match', new Highlight(...matchRanges));
          CSS.highlights.set('openchat-search-active', activeRange ? new Highlight(activeRange) : new Highlight());
        } catch {
          // Highlight registration can fail while the DOM is being replaced.
        }
      } else {
        setSearchMatchInfo(0, 0);
        CSS.highlights.delete('openchat-search-match');
        CSS.highlights.delete('openchat-search-active');
      }
    };

    scan();

    return () => {
      CSS.highlights.delete('openchat-search-match');
      CSS.highlights.delete('openchat-search-active');
    };
  }, [containerRef, query, activeTargetId, renderRevision, setSearchMatchInfo]);

  useEffect(() => {
    if (typeof CSS === 'undefined' || !('highlights' in CSS)) return;
    const ranges = rangesRef.current;
    if (ranges.length === 0) {
      CSS.highlights.delete('openchat-search-active');
      return;
    }

    const activeIndex = Math.min(searchMatchIndex, ranges.length - 1);
    const activeRange = ranges[activeIndex];
    const matchRanges = ranges.filter((_, index) => index !== activeIndex);
    try {
      CSS.highlights.set('openchat-search-match', new Highlight(...matchRanges));
      CSS.highlights.set('openchat-search-active', activeRange ? new Highlight(activeRange) : new Highlight());
    } catch {
      // Highlight registration can fail while the DOM is being replaced.
    }
  }, [searchMatchIndex]);

  // Scroll to active range when searchMatchIndex changes
  useEffect(() => {
    const range = rangesRef.current[searchMatchIndex];
    if (range) {
      const element = range.startContainer.parentElement;
      if (element) {
        element.scrollIntoView({ block: 'center', behavior: 'smooth' });
      }
    }
  }, [searchMatchIndex]);
}
