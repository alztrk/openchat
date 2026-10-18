import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/**
 * Applies the shared scroll-area behavior to a native div without changing its attributes.
 *
 * @param props Native div attributes and an optional additional class name.
 */
export function ScrollArea({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('openchat-scroll-area', className)} {...props} />;
}
