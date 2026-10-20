import { AlertCircle, CheckCircle2, Info, X } from 'lucide-react';
import type { HTMLAttributes, ReactNode } from 'react';

import { cn } from '@/shared/utils/cn';

/** Semantic color intent for the toast alert. */
export type ToastVariant = 'destructive' | 'info' | 'success' | 'warning';

export interface ToastProps extends HTMLAttributes<HTMLDivElement> {
  /** Visual intent variant; defaults to `destructive`. */
  variant?: ToastVariant;
  /** Toast content or message string. */
  message?: ReactNode;
  /** Optional callback invoked when the user dismisses the toast. */
  onClose?(): void;
  /** Optional action button attached to the toast. */
  action?: {
    label: string;
    onClick(): void;
  };
}

const variantStyles: Record<ToastVariant, { border: string; bg: string; iconColor: string; icon: typeof AlertCircle }> =
  {
    destructive: {
      border: 'border-destructive/30',
      bg: 'bg-raised',
      iconColor: 'text-destructive',
      icon: AlertCircle,
    },
    warning: {
      border: 'border-warning/30',
      bg: 'bg-raised',
      iconColor: 'text-warning',
      icon: AlertCircle,
    },
    info: {
      border: 'border-border',
      bg: 'bg-raised',
      iconColor: 'text-secondary-text',
      icon: Info,
    },
    success: {
      border: 'border-success/30',
      bg: 'bg-raised',
      iconColor: 'text-success',
      icon: CheckCircle2,
    },
  };

/**
 * Accessible toast alert primitive for floating, non-blocking operational feedback.
 */
export function Toast({
  className,
  variant = 'destructive',
  message,
  children,
  onClose,
  action,
  ...props
}: ToastProps) {
  const config = variantStyles[variant];
  const Icon = config.icon;

  return (
    <div
      aria-live="polite"
      className={cn(
        'flex items-center gap-3 rounded-card border px-4 py-2.5 shadow-md select-none',
        config.border,
        config.bg,
        className,
      )}
      role="alert"
      {...props}
    >
      <Icon aria-hidden="true" className={cn('size-4 shrink-0', config.iconColor)} />
      <div className="flex-1 text-sm font-medium text-text">{message ?? children}</div>
      {action ? (
        <button
          className="rounded-control px-2 py-1 text-xs font-semibold text-accent hover:bg-hover focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
          onClick={action.onClick}
          type="button"
        >
          {action.label}
        </button>
      ) : null}
      {onClose ? (
        <button
          aria-label="Close"
          className="rounded-control p-1 text-secondary-text hover:bg-hover hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
          onClick={onClose}
          type="button"
        >
          <X aria-hidden="true" size={14} />
        </button>
      ) : null}
    </div>
  );
}
