import type { InputHTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/** Native checkbox with the shared focus, border and accent treatment. */
export type CheckboxProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type'>;

export function Checkbox({ className, ...props }: CheckboxProps) {
  return (
    <input
      {...props}
      className={cn(
        'size-4 shrink-0 rounded border border-control-border accent-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50',
        className,
      )}
      type="checkbox"
    />
  );
}
