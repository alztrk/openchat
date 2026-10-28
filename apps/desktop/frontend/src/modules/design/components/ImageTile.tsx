import { LoaderCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { type GalleryImage, galleryPrompt, readGalleryImage } from '@/modules/design/components/designPageUtils';

export interface ImageTileProps {
  entry: GalleryImage;
  root: Element | null;
  onSelect(src: string): void;
}

export function ImageTile({ entry, root, onSelect }: ImageTileProps) {
  const { t } = useTranslation();
  const tileRef = useRef<HTMLButtonElement>(null);
  const [inRange, setInRange] = useState(false);
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const element = tileRef.current;
    if (!element || typeof IntersectionObserver === 'undefined') {
      setInRange(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        setInRange(entries.some((item) => item.isIntersecting));
      },
      { root, rootMargin: '480px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [root]);

  useEffect(() => {
    if (!inRange) {
      setSrc(null);
      return;
    }
    let active = true;
    setFailed(false);
    void readGalleryImage(entry)
      .then((value) => {
        if (active) setSrc(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [entry, inRange]);

  const label = galleryPrompt(entry);
  return (
    <Button
      aria-label={t('designOpenImage', { label: label.slice(0, 100) })}
      className="group design-image-tile relative min-h-0 w-full grid aspect-square min-w-0 overflow-hidden rounded-control border border-border bg-raised text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      onClick={() => {
        if (src) onSelect(src);
      }}
      ref={tileRef}
      size="small"
      type="button"
      variant="ghost"
    >
      {src ? (
        <img
          alt={label}
          className="size-full object-cover transition-transform duration-quick"
          loading="lazy"
          src={src}
        />
      ) : failed ? (
        <span className="flex h-full items-center justify-center px-3 text-center text-xs text-secondary-text">
          {t('designImageUnavailable')}
        </span>
      ) : (
        <span className="flex h-full items-center justify-center text-secondary-icon">
          <LoaderCircle aria-label={t('designImageLoading')} className="animate-spin" size={18} />
        </span>
      )}
      <span className="absolute inset-x-0 bottom-0 flex items-end justify-between gap-2 bg-gradient-to-t from-black/80 via-black/45 to-transparent px-3 pb-2 pt-12">
        <span
          aria-hidden="true"
          className="pointer-events-none line-clamp-3 min-w-0 flex-1 break-words text-left text-xs font-medium leading-4 text-white/75 opacity-0 transition-opacity duration-quick group-hover:opacity-75 group-focus-visible:opacity-75 motion-reduce:transition-none"
        >
          {label}
        </span>
        <span className="max-w-1/2 shrink-0 truncate text-xs font-medium text-white/70">
          {entry.source === 'design' ? entry.image.model : entry.image.conversationTitle}
        </span>
      </span>
    </Button>
  );
}
