import { useId, type InputHTMLAttributes } from 'react';

import { cn } from '@/shared/utils/cn';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export function Input({ className, error, id, label, ...props }: InputProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const errorId = `${inputId}-error`;

  return (
    <div className="grid gap-2">
      {label ? (
        <label className="text-sm font-medium text-text" htmlFor={inputId}>
          {label}
        </label>
      ) : null}
      <input
        id={inputId}
        aria-describedby={error ? errorId : props['aria-describedby']}
        aria-invalid={error ? true : props['aria-invalid']}
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
