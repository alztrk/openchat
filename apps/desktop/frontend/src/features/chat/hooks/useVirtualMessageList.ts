/**
 * Virtualizes conversation rows and preserves stable scroll positioning.
 *
 * Prefix offsets are rebuilt when messages, viewport width, or measured row heights change.
 * Scroll-only updates reuse those offsets and move the rendered window only near its edge.
 */
import type { RefCallback } from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { ChatMessage } from '@features/chat/data';
import { estimateWrappedLineCount } from '@features/chat/data/virtualMessageLayout';

const verticalPadding = 24;
const rowGap = 24;
const minimumDirectionalOverscanPixels = 640;
const maximumDirectionalOverscanPixels = 1_920;
const minimumReverseOverscanPixels = 320;
const maximumReverseOverscanPixels = 640;
const latestMessageThreshold = 96;

/** A message and its stable position in the unvirtualized conversation order. */
export interface VirtualMessageRow {
  index: number;
  message: ChatMessage;
}

/** Current scroll coordinates used to choose the virtual window. */
interface Viewport {
  height: number;
  width: number;
}

interface VisibleRange {
  start: number;
  end: number;
}

/**
 * Tracks measured row heights and returns the visible message slice with scroll controls.
 *
 * Changing `conversationId` clears retained measurements and resumes following the latest row.
 * The hook observes only mounted row elements; measured heights remain cached for offset
 * calculations until the conversation or viewport width changes.
 *
 * @param messages Messages in their rendered conversation order.
 * @param conversationId Identity of the active conversation, or `null` before one is selected.
 * @returns The scroll container ref, virtual rows and padding, row ref factory, follow-state
 * accessors, and controls for scrolling to a message or the conversation end.
 */
export function useVirtualMessageList(messages: ChatMessage[], conversationId: string | null) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rowElements = useRef(new Map<string, HTMLElement>());
  const rowCallbacks = useRef(new Map<string, RefCallback<HTMLElement>>());
  const rowIds = useRef(new WeakMap<Element, string>());
  const rowHeights = useRef(new Map<string, number>());
  const rowObserver = useRef<ResizeObserver | null>(null);
  const followLatestMessage = useRef(true);
  const lastScrollTop = useRef(0);
  const scrollDirection = useRef<'forward' | 'backward' | null>(null);
  const measuredViewportWidth = useRef(0);
  const visibleRangeRef = useRef<VisibleRange>({ start: 0, end: 0 });
  const [layoutRevision, setLayoutRevision] = useState(0);
  const [viewport, setViewport] = useState<Viewport>({ height: 0, width: 0 });
  const [visibleRange, setVisibleRange] = useState<VisibleRange>({ start: 0, end: 0 });

  /** Publishes viewport dimensions, which change far less often than scroll position. */
  const updateViewport = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    setViewport((current) => {
      const next = {
        height: element.clientHeight,
        width: element.clientWidth,
      };
      return current.height === next.height && current.width === next.width ? current : next;
    });
  }, []);

  useEffect(() => {
    let pendingFrame: number | null = null;
    const observer = new ResizeObserver((entries) => {
      let changed = false;
      for (const entry of entries) {
        const id = rowIds.current.get(entry.target);
        if (!id) continue;
        const blockSize = entry.borderBoxSize?.[0]?.blockSize;
        const height = Math.ceil(typeof blockSize === 'number' ? blockSize : entry.contentRect.height);
        if (height < 1) continue;
        const previous = rowHeights.current.get(id);
        if (previous === undefined || Math.abs(previous - height) > 1) {
          rowHeights.current.set(id, height);
          changed = true;
        }
      }
      if (changed) {
        if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);
        pendingFrame = requestAnimationFrame(() => {
          pendingFrame = null;
          setLayoutRevision((revision) => revision + 1);
        });
      }
    });
    rowObserver.current = observer;
    for (const element of rowElements.current.values()) observer.observe(element);
    return () => {
      if (pendingFrame !== null) cancelAnimationFrame(pendingFrame);
      observer.disconnect();
      rowObserver.current = null;
    };
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: Conversation identity resets the retained virtual measurements.
  useEffect(() => {
    followLatestMessage.current = true;
    lastScrollTop.current = 0;
    scrollDirection.current = null;
    rowHeights.current.clear();
    rowCallbacks.current.clear();
    measuredViewportWidth.current = 0;
    setLayoutRevision((revision) => revision + 1);
  }, [conversationId]);

  /** Reports whether new content should keep the viewport pinned to the latest row. */
  const isFollowingLatest = useCallback(() => followLatestMessage.current, []);

  /** Prevents later streamed content from moving the reader's current scroll position. */
  const stopFollowingLatest = useCallback(() => {
    followLatestMessage.current = false;
  }, []);

  useEffect(() => {
    const width = viewport.width;
    if (width < 1 || measuredViewportWidth.current === width) return;
    if (measuredViewportWidth.current > 0) {
      rowHeights.current.clear();
      setLayoutRevision((revision) => revision + 1);
    }
    measuredViewportWidth.current = width;
  }, [viewport.width]);

  /**
   * Returns a stable ref callback that observes a mounted row and releases it on unmount.
   *
   * @param id Stable message identifier used to retain its measured height.
   */
  const rowRef = useCallback((id: string): RefCallback<HTMLElement> => {
    const existing = rowCallbacks.current.get(id);
    if (existing) return existing;
    const callback: RefCallback<HTMLElement> = (element) => {
      const previous = rowElements.current.get(id);
      if (!element) {
        if (previous) {
          rowObserver.current?.unobserve(previous);
          rowIds.current.delete(previous);
          rowElements.current.delete(id);
        }
        if (rowCallbacks.current.get(id) === callback) rowCallbacks.current.delete(id);
        return;
      }
      if (previous && previous !== element) {
        rowObserver.current?.unobserve(previous);
        rowIds.current.delete(previous);
        rowElements.current.delete(id);
      }
      rowElements.current.set(id, element);
      rowIds.current.set(element, id);
      rowObserver.current?.observe(element);
    };
    rowCallbacks.current.set(id, callback);
    return callback;
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: ResizeObserver increments this revision to remeasure retained row heights.
  const offsets = useMemo(() => {
    const charsPerLine = Math.max(30, Math.floor(Math.min(768, viewport.width - 48) / 8));
    const nextOffsets = new Array<number>(messages.length + 1);
    nextOffsets[0] = 0;
    let cumulativeHeight = 0;
    messages.forEach((message, index) => {
      const measured = rowHeights.current.get(message.id);
      const height = measured ?? estimateMessageHeight(message, charsPerLine);
      cumulativeHeight += height;
      nextOffsets[index + 1] = cumulativeHeight;
    });
    return nextOffsets;
  }, [layoutRevision, messages, viewport.width]);

  /** Moves the rendered window only when the viewport approaches its buffered edge. */
  const updateVisibleRange = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    const totalHeight = offsetAt(offsets, messages.length);
    const viewportStart = Math.min(totalHeight, Math.max(0, element.scrollTop - verticalPadding));
    const viewportEnd = Math.min(
      totalHeight,
      Math.max(viewportStart, element.scrollTop + element.clientHeight - verticalPadding),
    );
    const current = visibleRangeRef.current;
    const currentStart = Math.min(current.start, messages.length);
    const currentEnd = Math.min(messages.length, Math.max(currentStart, current.end));
    const bufferBefore = viewportStart - offsetAt(offsets, currentStart);
    const bufferAfter = offsetAt(offsets, currentEnd) - viewportEnd;
    const isOutsideWindow = bufferBefore < 0 || bufferAfter < 0;
    const refreshThreshold = Math.min(
      maximumReverseOverscanPixels,
      Math.max(minimumReverseOverscanPixels, Math.ceil(element.clientHeight * 0.75)),
    );
    const isNearWindowEdge =
      scrollDirection.current === 'forward'
        ? bufferAfter <= refreshThreshold
        : scrollDirection.current === 'backward'
          ? bufferBefore <= refreshThreshold
          : bufferBefore <= refreshThreshold || bufferAfter <= refreshThreshold;
    if (!isOutsideWindow && !isNearWindowEdge) return;

    const directionalOverscan = Math.min(
      maximumDirectionalOverscanPixels,
      Math.max(minimumDirectionalOverscanPixels, Math.ceil(element.clientHeight * 1.5)),
    );
    const reverseOverscan = Math.min(
      maximumReverseOverscanPixels,
      Math.max(minimumReverseOverscanPixels, Math.ceil(element.clientHeight * 0.5)),
    );
    const overscanBefore =
      scrollDirection.current === 'forward'
        ? reverseOverscan
        : scrollDirection.current === 'backward'
          ? directionalOverscan
          : minimumDirectionalOverscanPixels;
    const overscanAfter =
      scrollDirection.current === 'backward'
        ? reverseOverscan
        : scrollDirection.current === 'forward'
          ? directionalOverscan
          : minimumDirectionalOverscanPixels;
    const visibleStart = findIndexAtOffset(
      offsets,
      Math.max(0, viewportStart - overscanBefore),
    );
    const visibleEnd = Math.min(
      messages.length,
      findIndexAtOffset(offsets, viewportEnd + overscanAfter) + 1,
    );
    if (currentStart === visibleStart && currentEnd === visibleEnd) return;
    const next = { start: visibleStart, end: visibleEnd };
    visibleRangeRef.current = next;
    setVisibleRange(next);
  }, [messages.length, offsets]);

  /** Tracks follow behavior on every scroll while rendering only when the virtual window changes. */
  const handleScroll = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    const scrollDelta = element.scrollTop - lastScrollTop.current;
    const distanceFromBottom = Math.max(0, element.scrollHeight - element.clientHeight - element.scrollTop);
    if (distanceFromBottom <= latestMessageThreshold) {
      followLatestMessage.current = true;
    } else if (scrollDelta < 0) {
      followLatestMessage.current = false;
    }
    if (scrollDelta > 0) scrollDirection.current = 'forward';
    else if (scrollDelta < 0) scrollDirection.current = 'backward';
    lastScrollTop.current = element.scrollTop;
    updateVisibleRange();
  }, [updateVisibleRange]);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    let pendingViewportFrame: number | null = null;
    const viewportObserver = new ResizeObserver(() => {
      if (pendingViewportFrame !== null) cancelAnimationFrame(pendingViewportFrame);
      pendingViewportFrame = requestAnimationFrame(() => {
        pendingViewportFrame = null;
        updateViewport();
        updateVisibleRange();
      });
    });
    viewportObserver.observe(element);
    element.addEventListener('scroll', handleScroll, { passive: true });
    updateViewport();
    updateVisibleRange();
    return () => {
      if (pendingViewportFrame !== null) cancelAnimationFrame(pendingViewportFrame);
      viewportObserver.disconnect();
      element.removeEventListener('scroll', handleScroll);
    };
  }, [handleScroll, updateViewport, updateVisibleRange]);

  useEffect(() => {
    updateVisibleRange();
  }, [updateVisibleRange]);

  const layout = useMemo(() => {
    const visibleStart = Math.min(visibleRange.start, messages.length);
    const visibleEnd = Math.min(messages.length, Math.max(visibleStart, visibleRange.end));
    const rows = messages.slice(visibleStart, visibleEnd).map((message, offset) => ({
      index: visibleStart + offset,
      message,
    }));

    return {
      bottomPadding: verticalPadding + Math.max(0, offsetAt(offsets, messages.length) - offsetAt(offsets, visibleEnd)),
      rows,
      topPadding: verticalPadding + offsetAt(offsets, visibleStart),
    };
  }, [messages, offsets, visibleRange.end, visibleRange.start]);

  /**
   * Moves to a message's estimated or measured position and centers it when space permits.
   *
   * @param id Stable identifier of the target message.
   * @returns `true` when the message exists and scrolling was scheduled; otherwise `false`.
   */
  const scrollToMessage = useCallback(
    (id: string): boolean => {
      const index = messages.findIndex((message) => message.id === id);
      const element = containerRef.current;
      if (index < 0 || !element) return false;
      followLatestMessage.current = false;
      const rowStart = offsetAt(offsets, index);
      const rowEnd = offsetAt(offsets, index + 1);
      const rowHeight = rowEnd - rowStart;
      const centeredOffset = rowStart - Math.max(0, (viewport.height - rowHeight) / 2);
      const targetScrollTop = verticalPadding + Math.max(0, centeredOffset);
      scrollDirection.current = targetScrollTop >= element.scrollTop ? 'forward' : 'backward';
      element.scrollTo({
        top: targetScrollTop,
        behavior: 'smooth',
      });
      updateVisibleRange();
      return true;
    },
    [messages, offsets, updateVisibleRange, viewport.height],
  );

  /** Pins the viewport to the current end and resumes following future content. */
  const scrollToBottom = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    followLatestMessage.current = true;
    element.scrollTop = element.scrollHeight;
    lastScrollTop.current = element.scrollTop;
    scrollDirection.current = 'forward';
    updateVisibleRange();
  }, [updateVisibleRange]);

  const totalHeight = offsetAt(offsets, messages.length);

  return {
    bottomPadding: layout.bottomPadding,
    containerRef,
    isFollowingLatest,
    rowRef,
    rows: layout.rows,
    scrollToBottom,
    scrollToMessage,
    stopFollowingLatest,
    topPadding: layout.topPadding,
    totalHeight,
  };
}

/**
 * Estimates a row's initial height until `ResizeObserver` supplies its measured height.
 *
 * @param message Message whose content and attachments determine the fallback height.
 * @param charsPerLine Estimated text wrapping width, in characters.
 * @returns Estimated row height in CSS pixels, including row spacing.
 */
function estimateMessageHeight(message: ChatMessage, charsPerLine: number): number {
  const lines = estimateWrappedLineCount(message.content, charsPerLine);
  const contentHeight = message.content ? lines * 26 + 48 : 64;
  const attachmentHeight = message.attachments.length * 132;
  const toolEstimate = message.toolActivities !== '[]' ? 96 : 0;
  const reasoningEstimate = message.reasoningSummaries !== '[]' ? 96 : 0;
  return Math.max(88, contentHeight + attachmentHeight + toolEstimate + reasoningEstimate + rowGap);
}

/**
 * Finds the row containing an offset in a prefix-sum array, clamped to its row range.
 *
 * @param offsets Prefix-summed row heights with an initial zero entry.
 * @param offset Scroll offset in CSS pixels.
 * @returns The containing row index, clamped to the valid row range.
 */
function findIndexAtOffset(offsets: number[], offset: number): number {
  let low = 0;
  let high = offsets.length - 1;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (offsetAt(offsets, middle + 1) <= offset) low = middle + 1;
    else high = middle;
  }
  return Math.min(low, Math.max(0, offsets.length - 2));
}

function offsetAt(offsets: number[], index: number): number {
  const offset = offsets[index];
  if (offset === undefined) throw new RangeError('Virtual message layout requested an unknown row offset.');
  return offset;
}
