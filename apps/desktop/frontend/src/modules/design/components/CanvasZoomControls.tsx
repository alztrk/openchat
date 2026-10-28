import { Minus, Plus, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';

interface CanvasZoomControlsProps {
  scale: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetView: () => void;
}

export function CanvasZoomControls({ scale, onZoomIn, onZoomOut, onResetView }: CanvasZoomControlsProps) {
  const { t } = useTranslation();

  return (
    <div className="absolute right-3 top-3 z-10 flex items-center gap-1 rounded-control border border-border bg-raised p-1 shadow-soft">
      <Button
        aria-label={t('designCanvasZoomOut')}
        disabled={scale <= 0.35}
        onClick={onZoomOut}
        size="icon"
        variant="ghost"
      >
        <Minus aria-hidden="true" size={16} />
      </Button>
      <span aria-live="polite" className="min-w-12 text-center text-xs tabular-nums text-secondary-text">
        {Math.round(scale * 100)}%
      </span>
      <Button
        aria-label={t('designCanvasZoomIn')}
        disabled={scale >= 2.5}
        onClick={onZoomIn}
        size="icon"
        variant="ghost"
      >
        <Plus aria-hidden="true" size={16} />
      </Button>
      <Button aria-label={t('designCanvasResetView')} onClick={onResetView} size="icon" variant="ghost">
        <RotateCcw aria-hidden="true" size={15} />
      </Button>
    </div>
  );
}
