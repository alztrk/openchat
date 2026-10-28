import {
  ArrowDown,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Download,
  LoaderCircle,
  RotateCcw,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button, ColorPicker, Select, Textarea } from '@/design-system';
import {
  type DesignCanvasBreakpoint,
  type DesignCanvasExportFormat,
  type DesignCanvasFrame,
  type DesignCanvasRevision,
  type DesignCanvasTokens,
  designCanvasFontFamilies,
  designCanvasSpacingUnits,
  designCanvasTypeScales,
} from '@/modules/design/data/designCanvas';
import type { ChatModel } from '@/modules/providers/data/chatModels';

export type CanvasColorTokenKey = 'background' | 'surface' | 'text' | 'primary' | 'accent' | 'border';
export type CanvasTokenTranslationKey =
  | 'designCanvasTokenBackground'
  | 'designCanvasTokenSurface'
  | 'designCanvasTokenText'
  | 'designCanvasTokenPrimary'
  | 'designCanvasTokenAccent'
  | 'designCanvasTokenBorder';

export const tokenKeys: CanvasColorTokenKey[] = ['background', 'surface', 'text', 'primary', 'accent', 'border'];
export const tokenLabelKeys: Record<CanvasColorTokenKey, CanvasTokenTranslationKey> = {
  background: 'designCanvasTokenBackground',
  surface: 'designCanvasTokenSurface',
  text: 'designCanvasTokenText',
  primary: 'designCanvasTokenPrimary',
  accent: 'designCanvasTokenAccent',
  border: 'designCanvasTokenBorder',
};

type ExportTranslationKey = 'designCanvasExportSvg' | 'designCanvasExportHtml' | 'designCanvasExportReact';

function exportLabel(format: DesignCanvasExportFormat): ExportTranslationKey {
  switch (format) {
    case 'svg':
      return 'designCanvasExportSvg';
    case 'html':
      return 'designCanvasExportHtml';
    case 'react':
      return 'designCanvasExportReact';
  }
}

export interface CanvasInspectorProps {
  selectedFrame: DesignCanvasFrame | null;
  previewFrame: DesignCanvasFrame | null;
  frameCount: number;
  saveState: 'idle' | 'saving' | 'saved' | 'error';
  onRetrySave: () => void;
  onOpenDeleteDialog: () => void;
  isBusy: boolean;
  isSaving: boolean;
  responsivePreviewWidth: number;
  onResponsivePreviewWidthChange: (width: number) => void;
  activeBreakpoint: DesignCanvasBreakpoint | null;
  selectedModel: ChatModel | null;
  requestMode: 'generate' | 'revise' | 'reflow' | null;
  onGenerateResponsiveLayout: () => void;
  selectedElements: Array<{ id: string }>;
  selectedElementId: string | null;
  onSelectElementId: (id: string) => void;
  selectedElement: { id: string } | null;
  onDownloadSelectedElement: () => void;
  tokenDraft: Partial<DesignCanvasTokens> | null;
  onSaveTokenDraft: () => void;
  selectedTokens: DesignCanvasTokens | null;
  onUpdateColorTokenDraft: (key: CanvasColorTokenKey, value: string) => void;
  onUpdateTokenDraft: <K extends keyof DesignCanvasTokens>(key: K, value: DesignCanvasTokens[K]) => void;
  onMoveSelectedFrame: (dx: number, dy: number) => void;
  onDownloadFrame: (format: DesignCanvasExportFormat) => void;
  comment: string;
  onCommentChange: (comment: string) => void;
  isPlacingComment: boolean;
  onTogglePlacingComment: () => void;
  commentPoint: { x: number; y: number } | null;
  onAddRevision: () => void;
  selectedCommentId: string | null;
  visibleRevisions: DesignCanvasRevision[];
  onRestoreRevision: (index: number) => void;
}

export function CanvasInspector({
  selectedFrame,
  previewFrame,
  frameCount,
  saveState,
  onRetrySave,
  onOpenDeleteDialog,
  isBusy,
  isSaving,
  responsivePreviewWidth,
  onResponsivePreviewWidthChange,
  activeBreakpoint,
  selectedModel,
  requestMode,
  onGenerateResponsiveLayout,
  selectedElements,
  selectedElementId,
  onSelectElementId,
  selectedElement,
  onDownloadSelectedElement,
  tokenDraft,
  onSaveTokenDraft,
  selectedTokens,
  onUpdateColorTokenDraft,
  onUpdateTokenDraft,
  onMoveSelectedFrame,
  onDownloadFrame,
  comment,
  onCommentChange,
  isPlacingComment,
  onTogglePlacingComment,
  commentPoint,
  onAddRevision,
  selectedCommentId,
  visibleRevisions,
  onRestoreRevision,
}: CanvasInspectorProps) {
  const { i18n, t } = useTranslation();

  return (
    <aside aria-label={t('designCanvasInspector')} className="design-vector-sidebar">
      <div className="flex items-center justify-between gap-3 border-b border-border pb-3">
        <div className="min-w-0">
          <p className="text-sm font-semibold text-text">{selectedFrame?.title ?? t('designCanvasInspector')}</p>
          <p className="mt-1 text-xs text-secondary-text">{t('designCanvasFrameCount', { count: frameCount })}</p>
        </div>
        {selectedFrame ? (
          <Button
            aria-label={t('designCanvasDeleteFrame')}
            disabled={isBusy || isSaving}
            onClick={onOpenDeleteDialog}
            size="icon"
            variant="ghost"
          >
            <Trash2 aria-hidden="true" size={16} />
          </Button>
        ) : null}
      </div>

      {saveState === 'saving' ? (
        <p aria-live="polite" className="flex items-center gap-2 text-xs text-secondary-text" role="status">
          <LoaderCircle aria-hidden="true" className="animate-spin" size={14} />
          {t('designCanvasSaving')}
        </p>
      ) : null}
      {saveState === 'saved' ? <p className="text-xs text-secondary-text">{t('designCanvasSaved')}</p> : null}
      {saveState === 'error' ? (
        <div className="grid gap-2" role="alert">
          <p className="text-xs text-destructive">{t('designCanvasSaveFailed')}</p>
          <Button onClick={onRetrySave} size="small" variant="secondary">
            <RotateCcw aria-hidden="true" size={14} />
            {t('retry')}
          </Button>
        </div>
      ) : null}

      {selectedFrame && previewFrame ? (
        <div className="grid gap-5">
          <section aria-labelledby="design-canvas-viewport-title" className="grid gap-2">
            <h2 className="text-sm font-semibold text-text" id="design-canvas-viewport-title">
              {t('designCanvasViewport')}
            </h2>
            <label className="grid gap-2 text-sm text-secondary-text" htmlFor="design-canvas-viewport-width">
              <span className="flex items-center justify-between gap-2">
                <span>{t('designCanvasViewportWidth')}</span>
                <output className="tabular-nums text-text" htmlFor="design-canvas-viewport-width">
                  {responsivePreviewWidth}px
                </output>
              </span>
              <input
                className="w-full accent-accent"
                id="design-canvas-viewport-width"
                max={1280}
                min={320}
                onChange={(event) => onResponsivePreviewWidthChange(Number(event.target.value))}
                type="range"
                value={responsivePreviewWidth}
              />
            </label>
            <p aria-live="polite" className="text-xs text-secondary-text" role="status">
              {activeBreakpoint
                ? t(
                    selectedFrame.responsiveLayouts[activeBreakpoint]
                      ? 'designCanvasBreakpointSaved'
                      : 'designCanvasBreakpointFallback',
                  )
                : t('designCanvasBreakpointBaseShown')}
            </p>
            {activeBreakpoint ? (
              <Button
                disabled={isBusy || isSaving || !selectedModel}
                loading={requestMode === 'reflow'}
                onClick={onGenerateResponsiveLayout}
                size="small"
                variant="secondary"
              >
                <Sparkles aria-hidden="true" size={14} />
                {t(
                  activeBreakpoint === 'mobile'
                    ? 'designCanvasGenerateMobileLayout'
                    : 'designCanvasGenerateTabletLayout',
                )}
              </Button>
            ) : null}
          </section>

          <section aria-labelledby="design-canvas-elements-title" className="grid gap-2">
            <h2 className="text-sm font-semibold text-text" id="design-canvas-elements-title">
              {t('designCanvasElements')}
            </h2>
            {selectedElements.length > 0 ? (
              <ol className="grid max-h-40 gap-1 overflow-y-auto">
                {selectedElements.map((element, index) => (
                  <li key={element.id}>
                    <Button
                      aria-pressed={element.id === selectedElementId}
                      className="w-full justify-start"
                      onClick={() => onSelectElementId(element.id)}
                      size="small"
                      variant={element.id === selectedElementId ? 'elevated' : 'ghost'}
                    >
                      {t('designCanvasElementLabel', { number: index + 1 })}
                    </Button>
                  </li>
                ))}
              </ol>
            ) : (
              <p className="text-xs text-secondary-text">{t('designCanvasNoElements')}</p>
            )}
            <Button
              disabled={isBusy || !selectedElement}
              onClick={onDownloadSelectedElement}
              size="small"
              variant="secondary"
            >
              <Download aria-hidden="true" size={14} />
              {t('designCanvasExportElement')}
            </Button>
          </section>

          <section aria-labelledby="design-canvas-tokens-title" className="grid gap-3">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-sm font-semibold text-text" id="design-canvas-tokens-title">
                {t('designCanvasTokens')}
              </h2>
              {tokenDraft ? (
                <Button disabled={isSaving} onClick={onSaveTokenDraft} size="small" variant="secondary">
                  {t('designCanvasSaveTokens')}
                </Button>
              ) : null}
            </div>
            <div className="grid grid-cols-2 gap-3">
              {tokenKeys.map((key) => (
                <ColorPicker
                  disabled={isBusy || isSaving}
                  key={key}
                  label={t(tokenLabelKeys[key])}
                  onChange={(value) => onUpdateColorTokenDraft(key, value)}
                  value={selectedTokens?.[key] ?? ''}
                />
              ))}
            </div>
            <Select
              disabled={isBusy || isSaving}
              label={t('designCanvasFontFamily')}
              onChange={(event) => {
                const fontFamily = designCanvasFontFamilies.find((candidate) => candidate === event.target.value);
                if (fontFamily) onUpdateTokenDraft('fontFamily', fontFamily);
              }}
              options={designCanvasFontFamilies.map((fontFamily) => ({ value: fontFamily, label: fontFamily }))}
              value={selectedTokens?.fontFamily ?? designCanvasFontFamilies[0]}
            />
            <Select
              disabled={isBusy || isSaving}
              label={t('designCanvasTypeScale')}
              onChange={(event) => {
                const typeScale = designCanvasTypeScales.find((candidate) => String(candidate) === event.target.value);
                if (typeScale !== undefined) onUpdateTokenDraft('typeScale', typeScale);
              }}
              options={designCanvasTypeScales.map((typeScale) => ({
                value: String(typeScale),
                label: `${typeScale} px`,
              }))}
              value={String(selectedTokens?.typeScale ?? designCanvasTypeScales[1])}
            />
            <Select
              disabled={isBusy || isSaving}
              label={t('designCanvasSpacingUnit')}
              onChange={(event) => {
                const spacingUnit = designCanvasSpacingUnits.find(
                  (candidate) => String(candidate) === event.target.value,
                );
                if (spacingUnit !== undefined) onUpdateTokenDraft('spacingUnit', spacingUnit);
              }}
              options={designCanvasSpacingUnits.map((spacingUnit) => ({
                value: String(spacingUnit),
                label: `${spacingUnit} px`,
              }))}
              value={String(selectedTokens?.spacingUnit ?? designCanvasSpacingUnits[1])}
            />
            <p className="text-xs text-secondary-text">{t('designCanvasTokensDescription')}</p>
          </section>

          <section aria-labelledby="design-canvas-move-title" className="grid gap-2">
            <h2 className="text-sm font-semibold text-text" id="design-canvas-move-title">
              {t('designCanvasPosition')}
            </h2>
            <div className="flex flex-wrap items-center gap-2">
              <Button
                aria-label={t('designCanvasMoveLeft')}
                disabled={isBusy || isSaving}
                onClick={() => onMoveSelectedFrame(-24, 0)}
                size="icon"
                variant="secondary"
              >
                <ArrowLeft aria-hidden="true" size={16} />
              </Button>
              <Button
                aria-label={t('designCanvasMoveUp')}
                disabled={isBusy || isSaving}
                onClick={() => onMoveSelectedFrame(0, -24)}
                size="icon"
                variant="secondary"
              >
                <ArrowUp aria-hidden="true" size={16} />
              </Button>
              <Button
                aria-label={t('designCanvasMoveDown')}
                disabled={isBusy || isSaving}
                onClick={() => onMoveSelectedFrame(0, 24)}
                size="icon"
                variant="secondary"
              >
                <ArrowDown aria-hidden="true" size={16} />
              </Button>
              <Button
                aria-label={t('designCanvasMoveRight')}
                disabled={isBusy || isSaving}
                onClick={() => onMoveSelectedFrame(24, 0)}
                size="icon"
                variant="secondary"
              >
                <ArrowRight aria-hidden="true" size={16} />
              </Button>
              <span className="text-xs tabular-nums text-secondary-text">
                {Math.round(selectedFrame.x)}, {Math.round(selectedFrame.y)}
              </span>
            </div>
          </section>

          <section aria-labelledby="design-canvas-export-title" className="grid gap-2">
            <h2 className="text-sm font-semibold text-text" id="design-canvas-export-title">
              {t('designCanvasExport')}
            </h2>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-3 2xl:grid-cols-1">
              {(['svg', 'html', 'react'] as const).map((format) => (
                <Button
                  key={format}
                  disabled={isBusy}
                  onClick={() => onDownloadFrame(format)}
                  size="small"
                  variant="secondary"
                >
                  <Download aria-hidden="true" size={14} />
                  {t(exportLabel(format))}
                </Button>
              ))}
            </div>
          </section>

          <section aria-labelledby="design-canvas-comments-title" className="grid gap-3">
            <h2 className="text-sm font-semibold text-text" id="design-canvas-comments-title">
              {t('designCanvasComments')}
            </h2>
            <label className="grid gap-2 text-sm font-medium text-text" htmlFor="design-canvas-comment">
              {t('designCanvasCommentLabel')}
              <Textarea
                className="min-h-20 w-full resize-y rounded-control border border-control-border bg-surface px-3 py-2 text-sm font-normal leading-relaxed text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
                disabled={isBusy || isSaving}
                id="design-canvas-comment"
                maxLength={2_000}
                onChange={(event) => onCommentChange(event.target.value)}
                placeholder={t('designCanvasCommentPlaceholder')}
                value={comment}
              />
            </label>
            <Button
              aria-pressed={isPlacingComment}
              disabled={isBusy || isSaving}
              onClick={onTogglePlacingComment}
              size="small"
              variant={isPlacingComment ? 'elevated' : 'secondary'}
            >
              {t(isPlacingComment ? 'designCanvasCancelPlacement' : 'designCanvasPlaceNote')}
            </Button>
            {isPlacingComment ? (
              <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                {t('designCanvasPlacementHint')}
              </p>
            ) : commentPoint ? (
              <p aria-live="polite" className="text-xs text-secondary-text" role="status">
                {t('designCanvasNotePinned')}
              </p>
            ) : null}
            <Button
              disabled={isBusy || isSaving || !selectedModel}
              loading={isBusy}
              onClick={onAddRevision}
              size="small"
              variant="primary"
            >
              <Sparkles aria-hidden="true" size={15} />
              {t('designCanvasRevise')}
            </Button>
            {selectedFrame.comments.length > 0 ? (
              <ol aria-label={t('designCanvasComments')} className="grid max-h-40 gap-2 overflow-y-auto">
                {selectedFrame.comments.map((item) => (
                  <li
                    className={`rounded-control border bg-surface px-3 py-2 text-sm text-text ${selectedCommentId === item.id ? 'border-accent' : 'border-border'}`}
                    key={item.id}
                  >
                    <p className="whitespace-pre-wrap break-words">{item.text}</p>
                    {item.point ? (
                      <span className="mt-1 block text-xs text-secondary-text">{t('designCanvasPinnedNote')}</span>
                    ) : null}
                    <time
                      className="mt-1 block text-xs text-secondary-text"
                      dateTime={new Date(item.createdAt).toISOString()}
                    >
                      {new Date(item.createdAt).toLocaleString(i18n.language)}
                    </time>
                  </li>
                ))}
              </ol>
            ) : (
              <p className="text-xs text-secondary-text">{t('designCanvasNoComments')}</p>
            )}
          </section>

          {visibleRevisions.length > 0 ? (
            <section aria-labelledby="design-canvas-history-title" className="grid gap-2">
              <h2 className="text-sm font-semibold text-text" id="design-canvas-history-title">
                {t('designCanvasHistory')}
              </h2>
              <ol className="grid gap-2">
                {visibleRevisions.map((revision, index) => (
                  <li className="flex items-center justify-between gap-2" key={revision.id}>
                    <span className="min-w-0 truncate text-xs text-secondary-text">
                      {t('designCanvasRevisionLabel', { number: visibleRevisions.length - index })}
                    </span>
                    <Button
                      disabled={isBusy || isSaving}
                      onClick={() => onRestoreRevision(index)}
                      size="small"
                      variant="ghost"
                    >
                      <RotateCcw aria-hidden="true" size={13} />
                      {t('designCanvasRestoreRevision')}
                    </Button>
                  </li>
                ))}
              </ol>
            </section>
          ) : null}
        </div>
      ) : (
        <div className="grid gap-2 py-3">
          <p className="text-sm font-medium text-text">{t('designCanvasNoSelection')}</p>
          <p className="text-sm text-secondary-text">{t('designCanvasNoSelectionDescription')}</p>
        </div>
      )}
    </aside>
  );
}
