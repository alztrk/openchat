import { defaultRangeExtractor, useVirtualizer, type Range as VirtualRange } from '@tanstack/react-virtual';
import { LoaderCircle } from 'lucide-react';
import {
  type FocusEvent as ReactFocusEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';

import type { GalleryImage } from '@features/design-workspace/components/designPageUtils';
import { ImageTile } from '@features/design-workspace/components/ImageTile';

const galleryGapRem = '0.75rem';
const initialGalleryGapPixels = 12;

interface DesignImageGalleryProps {
  entries: GalleryImage[];
  generationPlaceholderCount: number;
  onSelect(entry: GalleryImage, src: string): void;
  scrollElementRef: RefObject<HTMLDivElement | null>;
}

export function DesignImageGallery({
  entries,
  generationPlaceholderCount,
  onSelect,
  scrollElementRef,
}: DesignImageGalleryProps) {
  const { t } = useTranslation();
  const galleryRef = useRef<HTMLUListElement>(null);
  const pendingFocusIndex = useRef<number | null>(null);
  const [columns, setColumns] = useState(getGalleryColumnCount);
  const [galleryWidth, setGalleryWidth] = useState(0);
  const [galleryGap, setGalleryGap] = useState(initialGalleryGapPixels);
  const [scrollMargin, setScrollMargin] = useState(0);
  const [scrollRoot, setScrollRoot] = useState<Element | null>(null);
  const [focusedEntryKey, setFocusedEntryKey] = useState<string | null>(null);
  const count = generationPlaceholderCount + entries.length;
  const galleryIndexByKey = useMemo(
    () => new Map(entries.map((entry, index) => [entry.key, index])),
    [entries],
  );
  const focusedGalleryIndex = focusedEntryKey === null ? undefined : galleryIndexByKey.get(focusedEntryKey);
  const focusedVirtualIndex =
    focusedGalleryIndex === undefined ? null : generationPlaceholderCount + focusedGalleryIndex;

  const rangeExtractor = useCallback(
    (range: VirtualRange) => {
      const indices = defaultRangeExtractor(range);
      if (focusedVirtualIndex === null || focusedVirtualIndex >= count || indices.includes(focusedVirtualIndex)) {
        return indices;
      }
      return [...indices, focusedVirtualIndex].sort((left, right) => left - right);
    },
    [count, focusedVirtualIndex],
  );
  const getItemKey = useCallback(
    (index: number) => {
      if (index < generationPlaceholderCount) return `generation-placeholder-${index}`;
      const entry = entries[index - generationPlaceholderCount];
      if (!entry) throw new RangeError('Virtual gallery requested an unknown image.');
      return entry.key;
    },
    [entries, generationPlaceholderCount],
  );
  const estimateSize = useCallback(() => {
    const availableWidth = Math.max(0, galleryWidth - galleryGap * (columns - 1));
    return Math.max(1, availableWidth / columns);
  }, [columns, galleryGap, galleryWidth]);
  const virtualizer = useVirtualizer({
    count,
    directDomUpdates: true,
    directDomUpdatesMode: 'position',
    estimateSize,
    gap: galleryGap,
    getItemKey,
    getScrollElement: () => scrollElementRef.current,
    lanes: columns,
    overscan: 2,
    rangeExtractor,
    scrollMargin,
  });
  const virtualItems = virtualizer.getVirtualItems();

  const setGalleryRef = useCallback(
    (element: HTMLUListElement | null) => {
      galleryRef.current = element;
      virtualizer.containerRef(element);
    },
    [virtualizer.containerRef],
  );

  useEffect(() => {
    const updateColumns = () => setColumns(getGalleryColumnCount());
    window.addEventListener('resize', updateColumns);
    return () => window.removeEventListener('resize', updateColumns);
  }, []);

  useLayoutEffect(() => {
    const gallery = galleryRef.current;
    const scrollElement = scrollElementRef.current;
    if (!gallery || !scrollElement || entries.length + generationPlaceholderCount === 0) return;
    setScrollRoot(scrollElement);

    const updateLayoutMetrics = () => {
      const width = gallery.clientWidth;
      const gap = Number.parseFloat(window.getComputedStyle(gallery).columnGap);
      const nextScrollMargin = Math.max(
        0,
        gallery.getBoundingClientRect().top - scrollElement.getBoundingClientRect().top - scrollElement.clientTop + scrollElement.scrollTop,
      );

      setGalleryWidth((current) => (Math.abs(current - width) < 1 ? current : width));
      if (Number.isFinite(gap)) setGalleryGap((current) => (Math.abs(current - gap) < 0.5 ? current : gap));
      setScrollMargin((current) => (Math.abs(current - nextScrollMargin) < 1 ? current : nextScrollMargin));
    };

    updateLayoutMetrics();
    const observer = new ResizeObserver(updateLayoutMetrics);
    observer.observe(gallery);
    observer.observe(scrollElement);
    return () => observer.disconnect();
  }, [entries.length, generationPlaceholderCount, scrollElementRef]);

  useLayoutEffect(() => {
    const index = pendingFocusIndex.current;
    if (index === null) return;
    const button = galleryRef.current?.querySelector<HTMLButtonElement>(
      `button[data-gallery-index="${index}"]`,
    );
    if (!button) return;
    pendingFocusIndex.current = null;
    button.focus({ preventScroll: true });
  });

  const handleFocusCapture = (event: ReactFocusEvent<HTMLUListElement>) => {
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest<HTMLButtonElement>('button[data-gallery-index]');
    const index = Number(button?.dataset.galleryIndex);
    if (!Number.isInteger(index)) return;
    const entry = entries[index - generationPlaceholderCount];
    setFocusedEntryKey(entry?.key ?? null);
  };

  const handleBlurCapture = (event: ReactFocusEvent<HTMLUListElement>) => {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) return;
    setFocusedEntryKey(null);
  };

  const handleKeyDownCapture = (event: ReactKeyboardEvent<HTMLUListElement>) => {
    if (event.key !== 'Tab') return;
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest<HTMLButtonElement>('button[data-gallery-index]');
    const currentIndex = Number(button?.dataset.galleryIndex);
    if (!Number.isInteger(currentIndex)) return;
    const nextIndex = currentIndex + (event.shiftKey ? -1 : 1);
    const firstImageIndex = generationPlaceholderCount;
    const lastImageIndex = count - 1;
    if (nextIndex < firstImageIndex || nextIndex > lastImageIndex) return;
    if (galleryRef.current?.querySelector(`button[data-gallery-index="${nextIndex}"]`)) return;

    event.preventDefault();
    pendingFocusIndex.current = nextIndex;
    setFocusedEntryKey(entries[nextIndex - generationPlaceholderCount]?.key ?? null);
    virtualizer.scrollToIndex(nextIndex, { align: 'auto' });
  };

  return (
    <>
      {generationPlaceholderCount > 0 ? (
        <span className="sr-only" role="status">
          {t('designGenerationInProgress')}
        </span>
      ) : null}
      <ul
        className="relative m-0 w-full list-none p-0"
        onBlurCapture={handleBlurCapture}
        onFocusCapture={handleFocusCapture}
        onKeyDownCapture={handleKeyDownCapture}
        ref={setGalleryRef}
        style={{ columnGap: galleryGapRem, rowGap: galleryGapRem }}
      >
        {virtualItems.map((item) => {
          const isPlaceholder = item.index < generationPlaceholderCount;
          const entry = isPlaceholder ? undefined : entries[item.index - generationPlaceholderCount];
          const laneLeftPercent = (item.lane * 100) / columns;
          const laneGapPixels = (item.lane * galleryGap) / columns;
          const tileWidth = `calc(${100 / columns}% - ${(galleryGap * (columns - 1)) / columns}px)`;

          if (!isPlaceholder && !entry) {
            throw new RangeError('Virtual gallery requested an image outside the current result set.');
          }

          return (
            <li
              aria-hidden={isPlaceholder ? true : undefined}
              aria-posinset={isPlaceholder ? undefined : item.index - generationPlaceholderCount + 1}
              aria-setsize={isPlaceholder ? undefined : entries.length}
              className={isPlaceholder ? undefined : 'min-w-0'}
              data-index={item.index}
              key={item.key}
              ref={virtualizer.measureElement}
              role={isPlaceholder ? undefined : 'listitem'}
              style={{
                left: `${laneLeftPercent}%`,
                marginLeft: `${laneGapPixels}px`,
                position: 'absolute',
                width: tileWidth,
              }}
            >
              {isPlaceholder ? (
                <div className="design-image-tile relative grid aspect-square min-w-0 place-items-center overflow-hidden rounded-control border border-border bg-raised">
                  <span className="openchat-skeleton absolute inset-0 bg-selected" />
                  <span className="relative z-10 grid justify-items-center gap-3 text-accent">
                          <LoaderCircle aria-hidden="true" className="openchat-ai-ring-spinner" size={28} />
                    <span className="text-sm font-medium">{t('designGenerationInProgress')}</span>
                  </span>
                  <span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/75 to-transparent px-3 pb-2 pt-8 text-xs font-medium text-white">
                    {t('designGenerationInProgressHint')}
                  </span>
                </div>
              ) : entry ? (
                <ImageTile
                  entry={entry}
                  galleryIndex={item.index}
                  root={scrollRoot}
                  onSelect={(src) => onSelect(entry, src)}
                />
              ) : null}
            </li>
          );
        })}
      </ul>
    </>
  );
}

function getGalleryColumnCount(): number {
  if (typeof window === 'undefined') return 2;
  if (window.matchMedia('(min-width: 96rem)').matches) return 5;
  if (window.matchMedia('(min-width: 80rem)').matches) return 4;
  if (window.matchMedia('(min-width: 40rem)').matches) return 3;
  return 2;
}
