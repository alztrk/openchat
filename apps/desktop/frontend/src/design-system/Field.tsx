import type { AriaAttributes, ReactNode } from 'react';
import { useId } from 'react';

import { cn } from '@/shared/utils/cn';

export type FieldLayout = 'stacked' | 'inline';

export interface FieldControlProps {
  id: string;
  labelId: string;
  'aria-describedby'?: string;
  'aria-invalid'?: AriaAttributes['aria-invalid'];
}

interface FieldProps {
  'aria-describedby'?: string;
  'aria-invalid'?: AriaAttributes['aria-invalid'];
  className?: string;
  children(controlProps: FieldControlProps): ReactNode;
  description?: string;
  error?: string;
  id?: string;
  layout?: FieldLayout;
  label?: string;
  visuallyHiddenLabel?: boolean;
}

/** Shares field labels, helper text and validation feedback across native controls. */
export function Field({
  'aria-describedby': describedBy,
  'aria-invalid': ariaInvalid,
  children,
  className,
  description,
  error,
  id,
  layout = 'stacked',
  label,
  visuallyHiddenLabel = false,
}: FieldProps) {
  const generatedId = useId();
  const controlId = id ?? generatedId;
  const labelId = `${controlId}-label`;
  const descriptionId = `${controlId}-description`;
  const errorId = `${controlId}-error`;
  const descriptionIds = [describedBy, description ? descriptionId : undefined, error ? errorId : undefined]
    .filter((value): value is string => Boolean(value))
    .join(' ');

  return (
    <div className={cn(layout === 'inline' ? 'flex min-w-0 items-center gap-2' : 'grid min-w-0 gap-2', className)}>
      {label ? (
        <label
          className={visuallyHiddenLabel ? 'sr-only' : 'text-sm font-medium text-text'}
          htmlFor={controlId}
          id={labelId}
        >
          {label}
        </label>
      ) : null}
      {children({
        id: controlId,
        labelId,
        'aria-describedby': descriptionIds || undefined,
        'aria-invalid': error ? true : ariaInvalid,
      })}
      {description ? (
        <p className="text-xs text-secondary-text" id={descriptionId}>
          {description}
        </p>
      ) : null}
      {error ? (
        <p className="text-sm text-destructive" id={errorId} role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
