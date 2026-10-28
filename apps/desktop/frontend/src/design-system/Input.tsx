import type { InputHTMLAttributes } from 'react';
import { forwardRef } from 'react';

import { cn } from '@/shared/utils/cn';
import { Field, type FieldControlProps, type FieldLayout } from './Field';

export type InputAppearance = 'default' | 'bare';
export type InputDensity = 'default' | 'compact';

/** Native input attributes with optional field metadata and display presets. */
export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Visual treatment; `bare` is intended for inputs embedded in a toolbar or composite control. */
  appearance?: InputAppearance;
  /** Input density while retaining a usable touch target on narrow screens. */
  density?: InputDensity;
  /** Helper text associated with the input for visual and assistive-technology users. */
  description?: string;
  /** Error text announced through the input's described-by relationship. */
  error?: string;
  /** Classes applied to the field wrapper. */
  fieldClassName?: string;
  /** Field label and control arrangement. Use `none` when a surrounding control supplies semantics. */
  fieldLayout?: FieldLayout | 'none';
  /** Visible label; provide an accessible name with native ARIA attributes when omitted. */
  label?: string;
}

type InputControlProps = Omit<FieldControlProps, 'id' | 'labelId'> & { id?: string };

const appearanceClasses: Record<InputAppearance, string> = {
  default: 'rounded-control border border-control-border bg-surface',
  bare: 'rounded-control border border-transparent bg-transparent',
};

const densityClasses: Record<InputDensity, string> = {
  default: 'min-h-10 px-3 py-2',
  compact: 'min-h-10 px-2 py-1 text-sm sm:min-h-8',
};

/** Renders a labeled native input and keeps helper, error, and ref behavior shared. */
export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
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
  const renderInput = (controlProps: InputControlProps) => (
    <input
      {...props}
      {...controlProps}
      className={cn(
        'w-full text-text outline-none placeholder:text-secondary-text focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50',
        appearanceClasses[appearance],
        densityClasses[density],
        error && 'border-destructive',
        className,
      )}
      ref={ref}
    />
  );

  if (fieldLayout === 'none') {
    return renderInput({
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
      {renderInput}
    </Field>
  );
});
