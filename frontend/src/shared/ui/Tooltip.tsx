import { createPortal } from 'react-dom';
import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type PointerEvent,
  type ReactNode,
} from 'react';

const VIEWPORT_MARGIN = 8;
const TOOLTIP_GAP = 8;

interface TooltipPosition {
  top: number;
  left: number;
  placement: 'top' | 'bottom';
}

export interface TooltipProps {
  label: string;
  children: ReactNode;
}

export function Tooltip({ children, label }: TooltipProps) {
  const tooltipId = useId();
  const anchorRef = useRef<HTMLSpanElement>(null);
  const tooltipRef = useRef<HTMLSpanElement>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const [position, setPosition] = useState<TooltipPosition | null>(null);
  const isOpen = label.trim().length > 0 && (isHovered || isFocused);

  useLayoutEffect(() => {
    if (!isOpen) {
      setPosition(null);
      return;
    }

    const anchor = anchorRef.current;
    const tooltip = tooltipRef.current;
    if (!anchor || !tooltip) return;

    const updatePosition = () => {
      const anchorRect = anchor.getBoundingClientRect();
      const tooltipRect = tooltip.getBoundingClientRect();
      const viewportWidth = document.documentElement.clientWidth;
      const viewportHeight = document.documentElement.clientHeight;
      const availableBelow = viewportHeight - anchorRect.bottom - TOOLTIP_GAP - VIEWPORT_MARGIN;
      const availableAbove = anchorRect.top - TOOLTIP_GAP - VIEWPORT_MARGIN;
      const placement = tooltipRect.height <= availableBelow || availableBelow >= availableAbove
        ? 'bottom'
        : 'top';
      const desiredTop = placement === 'bottom'
        ? anchorRect.bottom + TOOLTIP_GAP
        : anchorRect.top - tooltipRect.height - TOOLTIP_GAP;
      const desiredLeft = anchorRect.left + (anchorRect.width - tooltipRect.width) / 2;
      const maxLeft = Math.max(VIEWPORT_MARGIN, viewportWidth - tooltipRect.width - VIEWPORT_MARGIN);
      const maxTop = Math.max(VIEWPORT_MARGIN, viewportHeight - tooltipRect.height - VIEWPORT_MARGIN);
      const nextPosition = {
        top: Math.min(maxTop, Math.max(VIEWPORT_MARGIN, desiredTop)),
        left: Math.min(maxLeft, Math.max(VIEWPORT_MARGIN, desiredLeft)),
        placement,
      } satisfies TooltipPosition;

      setPosition((current) => (
        current?.top === nextPosition.top
        && current.left === nextPosition.left
        && current.placement === nextPosition.placement
          ? current
          : nextPosition
      ));
    };

    updatePosition();
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);

    const resizeObserver = new ResizeObserver(updatePosition);
    resizeObserver.observe(anchor);
    resizeObserver.observe(tooltip);

    return () => {
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      resizeObserver.disconnect();
    };
  }, [isOpen]);

  const handlePointerEnter = (event: PointerEvent<HTMLSpanElement>) => {
    if (event.pointerType !== 'touch') setIsHovered(true);
  };

  const handleBlur = (event: FocusEvent<HTMLSpanElement>) => {
    const nextTarget = event.relatedTarget;
    if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
      setIsFocused(false);
    }
  };
  const portalTarget = typeof document === 'undefined'
    ? null
    : anchorRef.current?.closest('dialog[open]') ?? document.body;

  return (
    <span
      aria-describedby={isOpen ? tooltipId : undefined}
      className="inline-flex"
      onBlurCapture={handleBlur}
      onFocusCapture={() => setIsFocused(true)}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={() => setIsHovered(false)}
      ref={anchorRef}
    >
      {children}
      {isOpen && portalTarget
        ? createPortal(
          <span
            className="openchat-tooltip"
            data-placement={position?.placement}
            data-ready={position !== null}
            id={tooltipId}
            ref={tooltipRef}
            role="tooltip"
            style={{ left: position?.left ?? 0, top: position?.top ?? 0 }}
          >
            {label}
          </span>,
          portalTarget,
        )
        : null}
    </span>
  );
}
