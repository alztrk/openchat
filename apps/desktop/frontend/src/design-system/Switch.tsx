import type { ButtonHTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

/** Controlled props for the shared native-button switch. */
export interface SwitchProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onChange'> {
  /** Whether the switch is on. */
  checked: boolean;
  /** Called with the next checked state after activation. */
  onCheckedChange(checked: boolean): void;
}

/** Renders an accessible switch; callers supply its label through native ARIA attributes. */
export function Switch({ checked, className, onCheckedChange, ...props }: SwitchProps) {
  return (
    <button
      aria-checked={checked}
      className={cn(
        'openchat-switch relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-background',
        checked ? 'bg-accent' : 'bg-selected',
        className,
      )}
      onClick={() => onCheckedChange(!checked)}
      role="switch"
      type="button"
      {...props}
    >
      <span
        className={cn(
          'openchat-switch-thumb size-4 rounded-full bg-raised',
          checked ? 'translate-x-6' : 'translate-x-1',
        )}
      />
    </button>
  );
}
