import type { TextareaHTMLAttributes } from 'react';
import { forwardRef } from 'react';

import { cn } from '@/shared/utils/cn';
import { Field, type FieldControlProps, type FieldLayout } from './Field';

export type TextareaAppearance = 'default' | 'bare';
export type TextareaDensity = 'default' | 'compact';

/** Native textarea attributes with optional field metadata and display presets. */
export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  /** Visual treatment; `bare` is intended for controls embedded in a composite surface. */
  appearance?: TextareaAppearance;
  /** Minimum control height preset. */
  density?: TextareaDensity;
  /** Helper text associated with the textarea for visual and assistive-technology users. */
  description?: string;
  /** Error text announced through the textarea's described-by relationship. */
  error?: string;
  /** Classes applied to the field wrapper. */
  fieldClassName?: string;
  /** Field label and control arrangement. Use `none` when a surrounding control supplies semantics. */
  fieldLayout?: FieldLayout | 'none';
  /** Visible label; provide an accessible name with native ARIA attributes when omitted. */
  label?: string;
}

type TextareaControlProps = Omit<FieldControlProps, 'id' | 'labelId'> & { id?: string };

const appearanceClasses: Record<TextareaAppearance, string> = {
  default: 'rounded-control border border-control-border bg-surface',
  bare: 'rounded-control border border-transparent bg-transparent',
};

const densityClasses: Record<TextareaDensity, string> = {
  default: 'min-h-24 px-3 py-2',
  compact: 'min-h-10 px-2 py-1 text-sm',
};

/** Renders a labeled native textarea and associates helper/error text and refs. */
export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  {
    'aria-describedby': describedBy,
    'aria-invalid': ariaInvalid,
    appearance = 'default',
    className,
    density = 'default',
    description,
    error,
    fieldClassName,
    fieldLayout = 'stacked',
    id,
    label,
    ...props
  },
  ref,
) {
  const renderTextarea = (controlProps: TextareaControlProps) => (
    <textarea
      {...props}
      {...controlProps}
      className={cn(
        'w-full resize-y text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50',
        appearanceClasses[appearance],
        densityClasses[density],
        error && 'border-destructive',
        className,
      )}
      ref={ref}
    />
  );

  if (fieldLayout === 'none') {
    return renderTextarea({
      id,
      'aria-describedby': describedBy,
      'aria-invalid': error ? true : ariaInvalid,
    });
  }

  return (
    <Field
      aria-describedby={describedBy}
      aria-invalid={ariaInvalid}
      className={fieldClassName}
      description={description}
      error={error}
      id={id}
      label={label}
      layout={fieldLayout}
    >
      {renderTextarea}
    </Field>
  );
});
