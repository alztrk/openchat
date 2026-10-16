import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

export function Card({ className, ...props }: HTMLAttributes<HTMLElement>) {
  return (
    <section
      className={cn('rounded-card border border-border bg-surface text-text', className)}
      {...props}
    />
  );
}
