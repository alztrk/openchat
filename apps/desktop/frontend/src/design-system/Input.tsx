import { type InputHTMLAttributes, useId } from 'react';

import { cn } from '@/shared/utils/cn';

/** Native input attributes with optional visible label and inline error feedback. */
export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Visible label; provide an accessible name with native ARIA attributes when omitted. */
  label?: string;
  /** Error text announced through the input's described-by relationship. */
  error?: string;
}

/** Renders a labeled native input and keeps helper and error descriptions associated. */
export function Input({
  'aria-describedby': describedBy,
  'aria-invalid': ariaInvalid,
  className,
  error,
  id,
  label,
  ...props
}: InputProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;
  const descriptionIds = [describedBy, error ? errorId : undefined]
    .filter((descriptionId): descriptionId is string => Boolean(descriptionId))
    .join(' ');

  return (
    <div className="grid gap-2">
      {label ? (
        <label className="text-sm font-medium text-text" htmlFor={inputId}>
          {label}
        </label>
      ) : null}
      <input
        id={inputId}
        aria-describedby={descriptionIds || undefined}
        aria-invalid={error ? true : ariaInvalid}
        className={cn(
          'min-h-10 w-full rounded-control border border-control-border bg-surface px-3 py-2 text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50',
          error && 'border-destructive',
          className,
        )}
        {...props}
      />
      {error ? (
        <p className="text-sm text-destructive" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
