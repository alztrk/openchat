import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/** Semantic color variants for badges. */
export type BadgeVariant = 'neutral' | 'accent' | 'success' | 'warning' | 'destructive';

/** Supported badge size options. */
export type BadgeSize = 'default' | 'small';

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  /** Semantic color intent; defaults to `neutral`. */
  variant?: BadgeVariant;
  /** Size preset; defaults to `default`. */
  size?: BadgeSize;
}

const variantClasses: Record<BadgeVariant, string> = {
  neutral: 'border-border bg-selected/70 text-secondary-text',
  accent: 'border-accent/30 bg-accent/15 text-accent',
  success: 'border-success/30 bg-success/15 text-success',
  warning: 'border-warning/30 bg-warning/15 text-warning',
  destructive: 'border-destructive/30 bg-destructive/15 text-destructive',
};

const sizeClasses: Record<BadgeSize, string> = {
  default: 'min-h-6 px-2.5 py-0.5 text-xs',
  small: 'min-h-5 px-2 py-0.5 text-xs',
};

/**
 * Renders an accessible, rounded pill badge with semantic color variants.
 */
export function Badge({ className, variant = 'neutral', size = 'default', children, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center justify-center gap-1.5 rounded-full border font-medium leading-none whitespace-nowrap select-none',
        variantClasses[variant],
        sizeClasses[size],
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
}
