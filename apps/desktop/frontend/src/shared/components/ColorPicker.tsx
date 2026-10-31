import { Check, ChevronDown } from 'lucide-react';
import { type PointerEvent, useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

interface ColorPickerProps {
  disabled?: boolean;
  label: string;
  onChange(value: string): void;
  value: string;
}

interface PopoverPosition {
  left: number;
  maxHeight: number;
  top: number;
}

const presetColors = [
  '#FFFFFF',
  '#F3F4F6',
  '#D1D5DB',
  '#9CA3AF',
  '#4B5563',
  '#111827',
  '#000000',
  '#FCA5A5',
  '#EF4444',
  '#C2410C',
  '#FCD34D',
  '#EAB308',
  '#86EFAC',
  '#22C55E',
  '#0F766E',
  '#67E8F9',
  '#0EA5E9',
  '#1D4ED8',
  '#A5B4FC',
  '#7C3AED',
  '#F0ABFC',
  '#DB2777',
  '#E8DCCB',
  '#8FA7C7',
] as const;

/** Renders an in-app color picker with a two-dimensional color field and hex input. */
export function ColorPicker({ disabled = false, label, onChange, value }: ColorPickerProps) {
  const { t } = useTranslation();
  const id = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [hexDraft, setHexDraft] = useState(value.toUpperCase());
  const [position, setPosition] = useState<PopoverPosition | null>(null);
  const hsv = hexToHsv(value);

  const updateColor = (hue: number, saturation: number, brightness: number) => {
    const nextValue = hsvToHex(hue, saturation, brightness);
    setHexDraft(nextValue);
    onChange(nextValue.toLowerCase());
  };

  useEffect(() => {
    setHexDraft(value.toUpperCase());
  }, [value]);

  useLayoutEffect(() => {
    if (!isOpen) return;
    const updatePosition = () => {
      const trigger = triggerRef.current;
      const popover = popoverRef.current;
      if (!trigger || !popover) return;

      const triggerBounds = trigger.getBoundingClientRect();
      const popoverBounds = popover.getBoundingClientRect();
      const viewportPadding = 8;
      const left = Math.max(
        viewportPadding,
        Math.min(triggerBounds.left, window.innerWidth - popoverBounds.width - viewportPadding),
      );
      const availableBelow = window.innerHeight - triggerBounds.bottom - viewportPadding * 2;
      const availableAbove = triggerBounds.top - viewportPadding * 2;
      const openAbove = availableAbove > availableBelow;
      const availableHeight = Math.max(0, openAbove ? availableAbove : availableBelow);
      const requestedTop = openAbove
        ? triggerBounds.top - popoverBounds.height - viewportPadding
        : triggerBounds.bottom + viewportPadding;
      const maxTop = Math.max(viewportPadding, window.innerHeight - popoverBounds.height - viewportPadding);
      const top = Math.max(viewportPadding, Math.min(requestedTop, maxTop));

      setPosition((current) =>
        current?.left === left && current.top === top && current.maxHeight === availableHeight
          ? current
          : { left, maxHeight: availableHeight, top },
      );
    };

    updatePosition();
    const resizeObserver = new ResizeObserver(updatePosition);
    const trigger = triggerRef.current;
    const popover = popoverRef.current;
    if (trigger) resizeObserver.observe(trigger);
    if (popover) resizeObserver.observe(popover);
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const closeOnOutsidePointer = (event: globalThis.PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && !triggerRef.current?.contains(target) && !popoverRef.current?.contains(target)) {
        setIsOpen(false);
        setPosition(null);
      }
    };
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false);
        setPosition(null);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsidePointer);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [isOpen]);

  const updateColorFromPointer = (event: PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const saturation = Math.round(((event.clientX - rect.left) / rect.width) * 100);
    const brightness = Math.round(100 - ((event.clientY - rect.top) / rect.height) * 100);
    updateColor(hsv.hue, clamp(saturation, 0, 100), clamp(brightness, 0, 100));
  };

  return (
    <div className="grid min-w-0 gap-2 text-sm font-medium text-text">
      <span>{label}</span>
      <button
        aria-controls={`${id}-popover`}
        aria-expanded={isOpen}
        className="flex min-h-10 min-w-0 items-center gap-2 rounded-control border border-control-border bg-surface px-2 text-left outline-none hover:bg-surface-hover focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
        disabled={disabled}
        onClick={() => {
          setPosition(null);
          setIsOpen((open) => !open);
        }}
        ref={triggerRef}
        type="button"
      >
        <span
          aria-hidden="true"
          className="size-6 shrink-0 rounded border border-control-border"
          style={{ backgroundColor: value }}
        />
        <span className="min-w-0 flex-1 truncate font-mono text-xs font-normal text-secondary-text">
          {value.toUpperCase()}
        </span>
        <ChevronDown aria-hidden="true" className="shrink-0 text-secondary-icon" size={15} />
      </button>
      {isOpen
        ? createPortal(
            <div
              className="color-picker-popover grid gap-3 rounded-panel border border-control-border bg-raised p-3 text-text shadow-xl"
              id={`${id}-popover`}
              ref={popoverRef}
              role="dialog"
              aria-label={label}
              style={{
                left: position?.left ?? 0,
                maxHeight: position?.maxHeight,
                top: position?.top ?? 0,
                visibility: position ? 'visible' : 'hidden',
              }}
            >
              <div
                aria-hidden="true"
                className="relative h-36 touch-none cursor-crosshair rounded-control"
                onPointerDown={(event) => {
                  event.currentTarget.setPointerCapture(event.pointerId);
                  updateColorFromPointer(event);
                }}
                onPointerMove={(event) => {
                  if (event.buttons === 1) updateColorFromPointer(event);
                }}
                style={{
                  backgroundColor: `hsl(${hsv.hue} 100% 50%)`,
                  backgroundImage:
                    'linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent)',
                }}
              >
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-lg"
                  style={{ left: `${hsv.saturation}%`, top: `${100 - hsv.brightness}%` }}
                />
              </div>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor={`${id}-hue`}>
                <span className="flex items-center justify-between">
                  <span>{t('colorPickerHue')}</span>
                  <span className="font-mono">{Math.round(hsv.hue)}°</span>
                </span>
                <input
                  className="h-3 w-full cursor-pointer appearance-none rounded-full outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  id={`${id}-hue`}
                  max={360}
                  min={0}
                  onChange={(event) => updateColor(Number(event.target.value), hsv.saturation, hsv.brightness)}
                  style={{
                    background: 'linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)',
                  }}
                  type="range"
                  value={Math.round(hsv.hue)}
                />
              </label>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor={`${id}-saturation`}>
                <span className="flex items-center justify-between">
                  <span>{t('colorPickerSaturation')}</span>
                  <span className="font-mono">{Math.round(hsv.saturation)}%</span>
                </span>
                <input
                  className="h-1.5 w-full cursor-pointer accent-accent"
                  id={`${id}-saturation`}
                  max={100}
                  min={0}
                  onChange={(event) => updateColor(hsv.hue, Number(event.target.value), hsv.brightness)}
                  type="range"
                  value={Math.round(hsv.saturation)}
                />
              </label>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor={`${id}-brightness`}>
                <span className="flex items-center justify-between">
                  <span>{t('colorPickerBrightness')}</span>
                  <span className="font-mono">{Math.round(hsv.brightness)}%</span>
                </span>
                <input
                  className="h-1.5 w-full cursor-pointer accent-accent"
                  id={`${id}-brightness`}
                  max={100}
                  min={0}
                  onChange={(event) => updateColor(hsv.hue, hsv.saturation, Number(event.target.value))}
                  type="range"
                  value={Math.round(hsv.brightness)}
                />
              </label>
              <fieldset className="grid grid-cols-8 gap-1.5 border-0 p-0">
                <legend className="sr-only">{t('colorPickerPresets')}</legend>
                {presetColors.map((color) => (
                  <button
                    aria-label={t('colorPickerPresetColor', { color })}
                    className="relative grid size-6 place-items-center rounded border border-control-border outline-none hover:scale-110 focus-visible:ring-2 focus-visible:ring-focus"
                    key={color}
                    onClick={() => {
                      setHexDraft(color);
                      onChange(color.toLowerCase());
                    }}
                    style={{ backgroundColor: color }}
                    type="button"
                  >
                    {value.toUpperCase() === color ? (
                      <Check aria-hidden="true" className="text-white mix-blend-difference drop-shadow" size={13} />
                    ) : null}
                  </button>
                ))}
              </fieldset>
              <label className="grid gap-1.5 text-xs font-medium text-secondary-text" htmlFor={`${id}-hex`}>
                {t('colorPickerHex')}
                <input
                  autoComplete="off"
                  className="min-h-9 w-full rounded-control border border-control-border bg-surface px-2 font-mono text-sm text-text outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  id={`${id}-hex`}
                  maxLength={7}
                  onChange={(event) => {
                    const nextValue = event.target.value.toUpperCase();
                    setHexDraft(nextValue);
                    if (/^#[\da-fA-F]{6}$/.test(nextValue)) onChange(nextValue.toLowerCase());
                  }}
                  onBlur={() => setHexDraft(value.toUpperCase())}
                  spellCheck={false}
                  value={hexDraft}
                />
              </label>
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}

function hexToHsv(hex: string): { hue: number; saturation: number; brightness: number } {
  const normalized = /^#[\da-fA-F]{6}$/.test(hex) ? hex.slice(1) : '000000';
  const red = Number.parseInt(normalized.slice(0, 2), 16) / 255;
  const green = Number.parseInt(normalized.slice(2, 4), 16) / 255;
  const blue = Number.parseInt(normalized.slice(4, 6), 16) / 255;
  const maximum = Math.max(red, green, blue);
  const minimum = Math.min(red, green, blue);
  const delta = maximum - minimum;
  let hue = 0;
  if (delta > 0) {
    if (maximum === red) hue = ((green - blue) / delta) % 6;
    else if (maximum === green) hue = (blue - red) / delta + 2;
    else hue = (red - green) / delta + 4;
    hue *= 60;
    if (hue < 0) hue += 360;
  }
  return {
    hue,
    saturation: maximum === 0 ? 0 : (delta / maximum) * 100,
    brightness: maximum * 100,
  };
}

function hsvToHex(hue: number, saturation: number, brightness: number): string {
  const normalizedSaturation = saturation / 100;
  const normalizedBrightness = brightness / 100;
  const chroma = normalizedBrightness * normalizedSaturation;
  const section = hue / 60;
  const secondary = chroma * (1 - Math.abs((section % 2) - 1));
  const [red, green, blue] =
    section < 1
      ? [chroma, secondary, 0]
      : section < 2
        ? [secondary, chroma, 0]
        : section < 3
          ? [0, chroma, secondary]
          : section < 4
            ? [0, secondary, chroma]
            : section < 5
              ? [secondary, 0, chroma]
              : [chroma, 0, secondary];
  const match = normalizedBrightness - chroma;
  return `#${[red, green, blue]
    .map((channel) =>
      Math.round((channel + match) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`.toUpperCase();
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}
