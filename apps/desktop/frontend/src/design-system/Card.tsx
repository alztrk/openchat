import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

export type CardPadding = 'none' | 'compact' | 'default' | 'comfortable' | 'spacious';

export interface CardProps extends HTMLAttributes<HTMLElement> {
  /** Optional content spacing; defaults to `none` to preserve existing layouts. */
  padding?: CardPadding;
}

const paddingClasses: Record<CardPadding, string> = {
  none: '',
  compact: 'p-3',
  default: 'p-4',
  comfortable: 'p-5',
  spacious: 'p-6',
};

/**
 * Renders a semantic section with the standard surface, border, and card radius.
 *
 * @param props Native section attributes and an optional additional class name.
 */
export function Card({ className, padding = 'none', ...props }: CardProps) {
  return (
    <section
      className={cn('rounded-card border border-border bg-surface text-text', paddingClasses[padding], className)}
      {...props}
    />
  );
}
