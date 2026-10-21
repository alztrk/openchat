import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/** Supported shape presets for skeleton placeholders. */
export type SkeletonVariant = 'rounded' | 'circular' | 'text';

export interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  /** Visual geometry preset; defaults to `rounded`. */
  variant?: SkeletonVariant;
}

const variantClasses: Record<SkeletonVariant, string> = {
  rounded: 'rounded-control',
  circular: 'rounded-full',
  text: 'h-4 w-full rounded',
};

/**
 * Renders a layout-shift-free loading placeholder block with a gentle shimmer animation.
 */
export function Skeleton({ className, variant = 'rounded', ...props }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        'openchat-skeleton pointer-events-none select-none bg-hover/80 text-transparent',
        variantClasses[variant],
        className,
      )}
      {...props}
    />
  );
}
