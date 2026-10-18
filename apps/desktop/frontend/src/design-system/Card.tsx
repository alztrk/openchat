import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/**
 * Renders a semantic section with the standard surface, border, and card radius.
 *
 * @param props Native section attributes and an optional additional class name.
 */
export function Card({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={cn('rounded-card border border-border bg-surface text-text', className)} {...props} />;
}
