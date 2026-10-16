import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { RefCallback } from 'react';

import type { ChatMessage } from '@/features/chat/data/chatStorage';

const verticalPadding = 24;
const rowGap = 24;
const overscanPixels = 640;
const latestMessageThreshold = 96;

export interface VirtualMessageRow {
  index: number;
  message: ChatMessage;
}

interface Viewport {
  scrollTop: number;
  height: number;
  width: number;
}

export function useVirtualMessageList(messages: ChatMessage[], conversationId: string | null) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rowElements = useRef(new Map<string, HTMLDivElement>());
  const rowCallbacks = useRef(new Map<string, RefCallback<HTMLDivElement>>());
  const rowIds = useRef(new WeakMap<Element, string>());
  const rowHeights = useRef(new Map<string, number>());
  const rowObserver = useRef<ResizeObserver | null>(null);
  const followLatestMessage = useRef(true);
  const measuredViewportWidth = useRef(0);
  const [layoutRevision, setLayoutRevision] = useState(0);
  const [viewport, setViewport] = useState<Viewport>({ scrollTop: 0, height: 0, width: 0 });

  const updateViewport = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    setViewport((current) => {
      const next = {
        scrollTop: element.scrollTop,
        height: element.clientHeight,
        width: element.clientWidth,
      };
      return current.scrollTop === next.scrollTop &&
        current.height === next.height &&
        current.width === next.width
        ? current
        : next;
    });
  }, []);

  const handleScroll = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    followLatestMessage.current = Math.max(
      0,
      element.scrollHeight - element.clientHeight - element.scrollTop,
    ) <= latestMessageThreshold;
    updateViewport();
  }, [updateViewport]);

  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;
    const viewportObserver = new ResizeObserver(updateViewport);
    viewportObserver.observe(element);
    element.addEventListener('scroll', handleScroll, { passive: true });
    updateViewport();
    return () => {
      viewportObserver.disconnect();
      element.removeEventListener('scroll', handleScroll);
    };
  }, [handleScroll, updateViewport]);

  useEffect(() => {
    const observer = new ResizeObserver((entries) => {
      let changed = false;
      for (const entry of entries) {
        const id = rowIds.current.get(entry.target);
        if (!id) continue;
        const height = Math.ceil(entry.target.getBoundingClientRect().height);
        if (height < 1) continue;
        const previous = rowHeights.current.get(id);
        if (previous === undefined || Math.abs(previous - height) > 1) {
          rowHeights.current.set(id, height);
          changed = true;
        }
      }
      if (changed) setLayoutRevision((revision) => revision + 1);
    });
    rowObserver.current = observer;
    for (const element of rowElements.current.values()) observer.observe(element);
    return () => {
      observer.disconnect();
      rowObserver.current = null;
    };
  }, []);

  useEffect(() => {
    followLatestMessage.current = true;
    rowHeights.current.clear();
    rowCallbacks.current.clear();
    measuredViewportWidth.current = 0;
    setLayoutRevision((revision) => revision + 1);
  }, [conversationId]);

  const isFollowingLatest = useCallback(() => followLatestMessage.current, []);

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

  const rowRef = useCallback((id: string): RefCallback<HTMLDivElement> => {
    const existing = rowCallbacks.current.get(id);
    if (existing) return existing;
    const callback: RefCallback<HTMLDivElement> = (element) => {
      const previous = rowElements.current.get(id);
      if (previous && previous !== element) {
        rowObserver.current?.unobserve(previous);
        rowIds.current.delete(previous);
        rowElements.current.delete(id);
      }
      if (!element) return;
      rowElements.current.set(id, element);
      rowIds.current.set(element, id);
      rowObserver.current?.observe(element);
    };
    rowCallbacks.current.set(id, callback);
    return callback;
  }, []);

  const layout = useMemo(() => {
    const charsPerLine = Math.max(30, Math.floor(Math.min(768, viewport.width - 48) / 8));
    const offsets = new Array<number>(messages.length + 1);
    offsets[0] = 0;
    messages.forEach((message, index) => {
      const measured = rowHeights.current.get(message.id);
      const height = measured ?? estimateMessageHeight(message, charsPerLine);
      offsets[index + 1] = offsets[index] + height;
    });

    const visibleStart = findIndexAtOffset(offsets, Math.max(
      0,
      viewport.scrollTop - verticalPadding - overscanPixels,
    ));
    const visibleEnd = Math.min(
      messages.length,
      findIndexAtOffset(offsets, viewport.scrollTop + viewport.height - verticalPadding + overscanPixels) + 1,
    );
    const rows = messages.slice(visibleStart, visibleEnd).map((message, offset) => ({
      index: visibleStart + offset,
      message,
    }));

    return {
      bottomPadding: verticalPadding + Math.max(0, offsets[messages.length] - offsets[visibleEnd]),
      offsets,
      rows,
      topPadding: verticalPadding + offsets[visibleStart],
    };
  }, [layoutRevision, messages, viewport.height, viewport.scrollTop, viewport.width]);

  const scrollToMessage = useCallback((id: string): boolean => {
    const index = messages.findIndex((message) => message.id === id);
    const element = containerRef.current;
    if (index < 0 || !element) return false;
    followLatestMessage.current = false;
    const rowHeight = layout.offsets[index + 1] - layout.offsets[index];
    const centeredOffset = layout.offsets[index] - Math.max(0, (viewport.height - rowHeight) / 2);
    element.scrollTo({
      top: verticalPadding + Math.max(0, centeredOffset),
      behavior: 'smooth',
    });
    return true;
  }, [layout.offsets, messages, viewport.height]);

  const scrollToBottom = useCallback(() => {
    const element = containerRef.current;
    if (!element) return;
    followLatestMessage.current = true;
    element.scrollTop = element.scrollHeight;
  }, []);

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
  };
}

function estimateMessageHeight(message: ChatMessage, charsPerLine: number): number {
  const lines = message.content.split('\n').reduce((count, line) =>
    count + Math.max(1, Math.ceil(line.length / charsPerLine)), 0);
  const contentHeight = message.content ? lines * 26 + 48 : 64;
  const attachmentHeight = message.attachments.length * 132;
  return Math.max(88, contentHeight + attachmentHeight + rowGap);
}

function findIndexAtOffset(offsets: number[], offset: number): number {
  let low = 0;
  let high = offsets.length - 1;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (offsets[middle + 1] <= offset) low = middle + 1;
    else high = middle;
  }
  return Math.min(low, Math.max(0, offsets.length - 2));
}
