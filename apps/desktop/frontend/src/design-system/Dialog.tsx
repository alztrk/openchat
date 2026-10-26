import * as DialogPrimitive from '@radix-ui/react-dialog';
import type { ReactNode, RefObject } from 'react';

import { cn } from '@/shared/utils/cn';

/** Controlled modal dialog with a required accessible title. */
export interface DialogProps {
  /** Additional content classes. */
  className?: string;
  /** Whether the dialog is currently open. */
  open: boolean;
  /** Called when Escape requests a state change. */
  onOpenChange(open: boolean): void;
  /** Content announced as the dialog's accessible name. */
  title: ReactNode;
  /** Optional element that receives focus when the dialog opens. */
  initialFocusRef?: RefObject<HTMLElement | null>;
  /** Dialog body content. */
  children: ReactNode;
}

/** Renders a modal with managed focus, Escape dismissal, and an accessible title. */
export function Dialog({ children, className, initialFocusRef, onOpenChange, open, title }: DialogProps) {
  return (
    <DialogPrimitive.Root onOpenChange={onOpenChange} open={open}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="openchat-dialog-overlay" />
        <DialogPrimitive.Content
          aria-describedby={undefined}
          className={cn('openchat-dialog w-dialog', className)}
          onOpenAutoFocus={(event) => {
            if (!initialFocusRef?.current) return;
            event.preventDefault();
            initialFocusRef.current.focus();
          }}
          onPointerDownOutside={(event) => event.preventDefault()}
        >
          <DialogPrimitive.Title className="mb-4 text-section-title font-semibold">{title}</DialogPrimitive.Title>
          {children}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
