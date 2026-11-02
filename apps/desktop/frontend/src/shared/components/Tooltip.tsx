import { autoUpdate, flip, offset, shift, size, useFloating } from '@floating-ui/react-dom';
import { createPortal } from 'react-dom';
import { cloneElement, createContext, useContext, useEffect, useId, useRef, useState, type ReactElement } from 'react';

/** Content and anchor for a tooltip shown on pointer hover or keyboard focus. */
export interface TooltipProps {
  /** Explanatory text; empty or whitespace-only labels do not render a tooltip. */
  label: string;
  /** Element that triggers the tooltip. */
  children: ReactElement<{ 'aria-describedby'?: string }>;
  /** Preferred side for the tooltip; collision handling may flip it near the viewport edge. */
  side?: 'top' | 'right' | 'bottom' | 'left';
  /** Distance between the trigger and tooltip. */
  sideOffset?: number;
}

const VIEWPORT_PADDING = 12;
const HOVER_DELAY_MS = 500;
export const TooltipContext = createContext(false);

function isFlowContentElement(element: ReactElement): boolean {
  if (typeof element.type !== 'string') return false;
  return [
    'article',
    'aside',
    'blockquote',
    'details',
    'div',
    'dl',
    'fieldset',
    'figure',
    'footer',
    'form',
    'h1',
    'h2',
    'h3',
    'h4',
    'h5',
    'h6',
    'header',
    'main',
    'nav',
    'ol',
    'p',
    'pre',
    'section',
    'ul',
  ].includes(element.type);
}

export interface TitleTooltipProps {
  /** Native title text that should use the shared tooltip presentation. */
  title?: string;
  /** Trigger element. */
  children: ReactElement<{ 'aria-describedby'?: string }>;
}

/** Converts a component's title prop to the shared tooltip without nesting tooltips. */
export function TitleTooltip({ children, title }: TitleTooltipProps) {
  const isInsideTooltip = useContext(TooltipContext);
  if (!title || isInsideTooltip) return children;
  return <Tooltip label={title}>{children}</Tooltip>;
}

/** Positions tooltip content with viewport-aware flipping and shifting. */
export function Tooltip({
  children,
  label,
  side = 'top',
  sideOffset = 8,
}: TooltipProps) {
  const tooltipId = useId();
  const [isOpen, setIsOpen] = useState(false);
  const hoverTimerRef = useRef<number | null>(null);
  const isPointerInsideRef = useRef(false);
  const isFocusInsideRef = useRef(false);
  const floating = useFloating({
    open: isOpen,
    placement: side,
    strategy: 'fixed',
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(sideOffset),
      flip({ padding: VIEWPORT_PADDING }),
      shift({ crossAxis: true, padding: VIEWPORT_PADDING }),
      size({
        padding: VIEWPORT_PADDING,
        apply({ availableWidth, availableHeight, elements }) {
          elements.floating.style.maxWidth = `${Math.max(0, availableWidth)}px`;
          elements.floating.style.maxHeight = `${Math.max(0, availableHeight)}px`;
        },
      }),
    ],
  });

  useEffect(
    () => () => {
      if (hoverTimerRef.current !== null) window.clearTimeout(hoverTimerRef.current);
    },
    [],
  );

  if (!label.trim()) return children;

  const closeIfInactive = () => {
    if (!isPointerInsideRef.current && !isFocusInsideRef.current) setIsOpen(false);
  };

  const existingDescription = children.props['aria-describedby'];
  const Anchor = isFlowContentElement(children) ? 'div' : 'span';
  const describedBy = isOpen
    ? [existingDescription, tooltipId].filter((value): value is string => Boolean(value)).join(' ')
    : existingDescription;
  const trigger = cloneElement(children, { 'aria-describedby': describedBy });

  return (
    <TooltipContext.Provider value>
      <Anchor
        className="openchat-tooltip-anchor"
        onBlurCapture={(event) => {
          const nextTarget = event.relatedTarget;
          if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) return;
          isFocusInsideRef.current = false;
          closeIfInactive();
        }}
        onFocusCapture={() => {
          isFocusInsideRef.current = true;
          setIsOpen(true);
        }}
        onKeyDownCapture={(event) => {
          if (event.key !== 'Escape') return;
          isPointerInsideRef.current = false;
          isFocusInsideRef.current = false;
          setIsOpen(false);
        }}
        onPointerEnter={(event) => {
          if (event.pointerType === 'touch') return;
          isPointerInsideRef.current = true;
          if (hoverTimerRef.current !== null) window.clearTimeout(hoverTimerRef.current);
          hoverTimerRef.current = window.setTimeout(() => setIsOpen(true), HOVER_DELAY_MS);
        }}
        onPointerLeave={() => {
          isPointerInsideRef.current = false;
          if (hoverTimerRef.current !== null) {
            window.clearTimeout(hoverTimerRef.current);
            hoverTimerRef.current = null;
          }
          closeIfInactive();
        }}
        ref={(anchor) => {
          if (!anchor) {
            floating.refs.setReference(null);
            return;
          }

          const getRange = () => {
            const range = document.createRange();
            range.selectNodeContents(anchor);
            return range;
          };
          const rangeRect = getRange().getBoundingClientRect();
          if (rangeRect.width > 0 || rangeRect.height > 0) {
            floating.refs.setReference({
              contextElement: anchor,
              getBoundingClientRect: () => getRange().getBoundingClientRect(),
              getClientRects: () => getRange().getClientRects(),
            });
            return;
          }

          const triggerElement = anchor.firstElementChild;
          floating.refs.setReference(triggerElement instanceof HTMLElement ? triggerElement : null);
        }}
      >
        {trigger}
        {isOpen
          ? createPortal(
              <div
                className="openchat-tooltip-positioner"
                ref={floating.refs.setFloating}
                style={{ ...floating.floatingStyles, visibility: floating.isPositioned ? 'visible' : 'hidden' }}
              >
                <div className="openchat-tooltip" id={tooltipId} role="tooltip">
                  {label}
                </div>
              </div>,
              document.body,
            )
          : null}
      </Anchor>
    </TooltipContext.Provider>
  );
}
