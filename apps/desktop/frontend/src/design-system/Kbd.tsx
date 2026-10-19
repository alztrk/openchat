import type { HTMLAttributes, ReactNode } from 'react';

import { cn } from '@/shared/utils/cn';

export interface KbdProps extends HTMLAttributes<HTMLElement> {
  /** Individual shortcut key or sequence of keys to render. */
  keys?: string | string[];
  /** Optional inner content when keys is not provided. */
  children?: ReactNode;
}

const keySymbolMap: Record<string, string> = {
  cmd: 'Cmd',
  command: 'Cmd',
  ctrl: 'Ctrl',
  control: 'Ctrl',
  shift: 'Shift',
  enter: 'Enter',
  return: 'Enter',
  alt: 'Alt',
  option: 'Alt',
  esc: 'Esc',
  escape: 'Esc',
  tab: 'Tab',
  backspace: 'Backspace',
  delete: 'Del',
  space: 'Space',
  up: '↑',
  down: '↓',
  left: '←',
  right: '→',
};

function formatKey(key: string): string {
  const normalized = key.trim().toLowerCase();
  return keySymbolMap[normalized] ?? key.trim();
}

/**
 * Renders an accessible keyboard shortcut badge with subtle elevation and border.
 */
export function Kbd({
  className,
  keys,
  children,
  ...props
}: KbdProps) {
  const keyList = Array.isArray(keys) ? keys : keys ? [keys] : null;

  if (keyList && keyList.length > 0) {
    return (
      <kbd
        className={cn(
          'inline-flex items-center gap-1 font-code text-xs text-secondary-text select-none',
          className,
        )}
        {...props}
      >
        {keyList.map((k, index) => (
          <span key={`${k}-${index}`} className="inline-flex items-center gap-1">
            {index > 0 ? <span className="text-secondary-text/60">+</span> : null}
            <kbd className="inline-flex h-5 min-w-5 items-center justify-center rounded border border-border border-b-control-border/40 bg-raised px-1.5 py-0.5 font-code text-xs font-medium text-text shadow-xs">
              {formatKey(k)}
            </kbd>
          </span>
        ))}
      </kbd>
    );
  }

  return (
    <kbd
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded border border-border border-b-control-border/40 bg-raised px-1.5 py-0.5 font-code text-xs font-medium text-text shadow-xs select-none',
        className,
      )}
      {...props}
    >
      {children}
    </kbd>
  );
}
