export interface ModelPickerAnchor {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface ModelPickerPosition {
  top: number;
  left: number;
  width: number;
  maxHeight: number;
  placement: 'above' | 'below';
  alignment: 'left' | 'right';
}

export function calculateModelPickerPosition(
  anchor: ModelPickerAnchor,
  panel: { width: number; height: number },
  viewport: { width: number; height: number },
): ModelPickerPosition {
  const margin = 8;
  const gap = 8;
  const maximumWidth = Math.max(0, viewport.width - margin * 2);
  const width = Math.min(panel.width, maximumWidth);
  const availableBelow = Math.max(0, viewport.height - anchor.bottom - gap - margin);
  const availableAbove = Math.max(0, anchor.top - gap - margin);
  const placement = availableBelow >= panel.height || availableBelow >= availableAbove ? 'below' : 'above';
  const availableHeight = placement === 'below' ? availableBelow : availableAbove;
  const maxHeight = Math.min(panel.height, availableHeight);
  const alignment = anchor.left + width <= viewport.width - margin ? 'left' : 'right';
  const requestedLeft = alignment === 'left' ? anchor.left : anchor.right - width;
  const left = Math.max(margin, Math.min(viewport.width - width - margin, requestedLeft));
  const top = placement === 'below' ? anchor.bottom + gap : anchor.top - gap - maxHeight;

  return { top, left, width, maxHeight, placement, alignment };
}
