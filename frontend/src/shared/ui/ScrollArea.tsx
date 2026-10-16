import type { HTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

export function ScrollArea({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('openchat-scroll-area', className)} {...props} />;
}
