import { type RefObject, useEffect } from 'react';

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
  useEffect(() => {
    if (typeof CSS === 'undefined' || !('highlights' in CSS)) return;

    const trimmed = query.trim().toLowerCase();
    if (!trimmed) {
      CSS.highlights.delete('openchat-search-match');
      CSS.highlights.delete('openchat-search-active');
      return;
    }

    const container = containerRef.current;
    if (!container) return;

    const matchRanges: Range[] = [];
    const activeRanges: Range[] = [];

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

    let currentNode: Node | null;
    while ((currentNode = walker.nextNode())) {
      const text = currentNode.nodeValue ?? '';
      const lower = text.toLowerCase();
      let startIndex = 0;

      while (startIndex < lower.length) {
        const foundIndex = lower.indexOf(trimmed, startIndex);
        if (foundIndex === -1) break;

        try {
          const range = new Range();
          range.setStart(currentNode, foundIndex);
          range.setEnd(currentNode, foundIndex + trimmed.length);

          const messageArticle = currentNode.parentElement?.closest('[id^="openchat-message-"]');
          const isActiveMessage = Boolean(
            activeTargetId && messageArticle?.id === `openchat-message-${activeTargetId}`,
          );

          if (isActiveMessage) {
            activeRanges.push(range);
          } else {
            matchRanges.push(range);
          }
        } catch {
          // Boundary adjustments during dynamic render can be safely ignored
        }

        startIndex = foundIndex + Math.max(1, trimmed.length);
      }
    }

    try {
      CSS.highlights.set('openchat-search-match', new Highlight(...matchRanges));
      CSS.highlights.set('openchat-search-active', new Highlight(...activeRanges));
    } catch {
      // In case Highlight constructor fails in test or restricted environments
    }

    return () => {
      CSS.highlights.delete('openchat-search-match');
      CSS.highlights.delete('openchat-search-active');
    };
  }, [containerRef, query, activeTargetId, renderRevision]);
}
