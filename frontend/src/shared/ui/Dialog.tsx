import { useEffect, useId, useRef, type DialogHTMLAttributes, type ReactNode } from 'react';

import { cn } from '@/shared/utils/cn';

export interface DialogProps
  extends Omit<DialogHTMLAttributes<HTMLDialogElement>, 'open' | 'onClose' | 'title'> {
  open: boolean;
  onOpenChange(open: boolean): void;
  title: ReactNode;
  children: ReactNode;
}

export function Dialog({
  children,
  className,
  onOpenChange,
  open,
  title,
  ...props
}: DialogProps) {
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
      className={cn('openchat-dialog w-[min(36rem,calc(100vw-2rem))]', className)}
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
