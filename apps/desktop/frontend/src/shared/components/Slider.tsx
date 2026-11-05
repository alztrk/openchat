import type { InputHTMLAttributes } from 'react';
import { forwardRef } from 'react';

import { cn } from '@shared/lib/utils/cn';
import { Field, type FieldLayout } from './Field';
import { TitleTooltip } from './Tooltip';

/** Native range input with optional field metadata and consistent focus behavior. */
export interface SliderProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> {
  description?: string;
  error?: string;
  fieldClassName?: string;
  fieldLayout?: FieldLayout;
  label?: string;
}

/** Renders a native range input while preserving browser range and keyboard semantics. */
export const Slider = forwardRef<HTMLInputElement, SliderProps>(function Slider(
  {
    'aria-describedby': describedBy,
    'aria-invalid': ariaInvalid,
    className,
    description,
    error,
    fieldClassName,
    fieldLayout = 'stacked',
    id,
    label,
    title,
    ...props
  },
  ref,
) {
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
      {({
        id: controlId,
        'aria-describedby': controlDescription,
        'aria-invalid': controlInvalid,
      }) => (
        <TitleTooltip title={title}>
          <input
            {...props}
            id={controlId}
            aria-describedby={controlDescription}
            aria-invalid={controlInvalid}
            className={cn(
              'w-full cursor-pointer accent-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50',
              className,
            )}
            ref={ref}
            type="range"
          />
        </TitleTooltip>
      )}
    </Field>
  );
});
