import { type DialogHTMLAttributes, type ReactNode, useEffect, useId, useRef } from 'react';

import { cn } from '@/shared/utils/cn';

/** Controlled native dialog with a required accessible title and close callback. */
export interface DialogProps extends Omit<DialogHTMLAttributes<HTMLDialogElement>, 'open' | 'onClose' | 'title'> {
  /** Whether the dialog is currently open. */
  open: boolean;
  /** Called when Escape or a native close event requests a state change. */
  onOpenChange(open: boolean): void;
  /** Content announced as the dialog's accessible name. */
  title: ReactNode;
  /** Dialog body content. */
  children: ReactNode;
}

/** Keeps the native modal dialog synchronized with its controlled `open` state. */
export function Dialog({ children, className, onOpenChange, open, title, ...props }: DialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      aria-labelledby={titleId}
      className={cn('openchat-dialog w-dialog', className)}
      onCancel={(event) => {
        event.preventDefault();
        onOpenChange(false);
      }}
      onClose={() => onOpenChange(false)}
      ref={dialogRef}
      {...props}
    >
      <h2 className="mb-4 text-section-title font-semibold" id={titleId}>
        {title}
      </h2>
      {children}
    </dialog>
  );
}
