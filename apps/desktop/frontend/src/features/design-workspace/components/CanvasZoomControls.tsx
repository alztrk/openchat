import { Minus, Plus, RotateCcw } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@shared/components';

interface CanvasZoomControlsProps {
  scale: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetView: () => void;
  onSetZoom: (scale: number) => void;
}

export function CanvasZoomControls({ scale, onZoomIn, onZoomOut, onResetView, onSetZoom }: CanvasZoomControlsProps) {
  const { t } = useTranslation();
  const [zoomDraft, setZoomDraft] = useState(String(Math.round(scale * 100)));

  useEffect(() => {
    setZoomDraft(String(Math.round(scale * 100)));
  }, [scale]);

  const commitZoom = () => {
    const parsed = Number(zoomDraft);
    if (!Number.isFinite(parsed) || zoomDraft.trim() === '') {
      setZoomDraft(String(Math.round(scale * 100)));
      return;
    }
    const percent = Math.max(5, Math.min(800, parsed));
    setZoomDraft(String(percent));
    onSetZoom(percent / 100);
  };

  return (
    <div aria-label={t('designEditorZoom')} className="design-editor-zoom-controls flex items-center gap-1" role="toolbar">
      <Button
        aria-label={t('designCanvasZoomOut')}
        disabled={scale <= 0.05}
        onClick={onZoomOut}
        size="icon"
        variant="ghost"
      >
        <Minus aria-hidden="true" size={16} />
      </Button>
      <label className="design-editor-zoom-input-wrap">
        <input
          aria-label={t('designEditorZoom')}
          className="design-editor-zoom-input"
          inputMode="decimal"
          max={800}
          min={5}
          onBlur={commitZoom}
          onChange={(event) => setZoomDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur();
          }}
          type="number"
          value={zoomDraft}
        />
        <span aria-hidden="true">%</span>
      </label>
      <Button
        aria-label={t('designCanvasZoomIn')}
        disabled={scale >= 8}
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
