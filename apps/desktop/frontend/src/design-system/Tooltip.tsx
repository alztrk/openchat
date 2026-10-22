import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import type { ReactElement } from 'react';

/** Content and anchor for a tooltip shown on pointer hover or keyboard focus. */
export interface TooltipProps {
  /** Explanatory text; empty or whitespace-only labels do not render a tooltip. */
  label: string;
  /** Element that triggers the tooltip. */
  children: ReactElement;
  /** Preferred side for the tooltip; collision handling may flip it near the viewport edge. */
  side?: 'top' | 'right' | 'bottom' | 'left';
  /** Distance between the trigger and tooltip. */
  sideOffset?: number;
}

/** Positions tooltip content safely and supports focus as well as hover. */
export function Tooltip({ children, label, side, sideOffset = 8 }: TooltipProps) {
  if (!label.trim()) return <>{children}</>;

  return (
    <TooltipPrimitive.Provider delayDuration={500} skipDelayDuration={150}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content className="openchat-tooltip" collisionPadding={8} side={side} sideOffset={sideOffset}>
            {label}
            <TooltipPrimitive.Arrow className="openchat-tooltip-arrow" />
          </TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
