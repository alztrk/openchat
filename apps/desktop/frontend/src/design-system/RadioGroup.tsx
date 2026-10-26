import type { ReactNode } from 'react';

import { cn } from '@/shared/utils/cn';
import { useId } from 'react';

export interface RadioOption {
  value: string;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}

export type RadioGroupVariant = 'list' | 'cards' | 'segmented';
export type RadioGroupOrientation = 'horizontal' | 'vertical';

export interface RadioGroupProps {
  'aria-describedby'?: string;
  disabled?: boolean;
  label: string;
  name?: string;
  onValueChange(value: string): void;
  options: readonly RadioOption[];
  orientation?: RadioGroupOrientation;
  value: string;
  variant?: RadioGroupVariant;
}

/** Renders an accessible native radio group with reusable list or card choices. */
export function RadioGroup({
  'aria-describedby': describedBy,
  disabled = false,
  label,
  name,
  onValueChange,
  orientation = 'vertical',
  options,
  value,
  variant = 'list',
}: RadioGroupProps) {
  const generatedName = useId();

  return (
    <fieldset aria-describedby={describedBy} className="grid min-w-0 gap-2" disabled={disabled}>
      <legend className="sr-only">{label}</legend>
      <div
        className={cn(
          'grid gap-2',
          variant === 'segmented' && 'w-full gap-1 rounded-button border border-border bg-raised p-1',
          variant === 'list' && orientation === 'horizontal' && 'grid-flow-col auto-cols-fr',
          variant === 'cards' && orientation === 'horizontal' && 'grid-flow-col auto-cols-fr',
        )}
        style={
          variant === 'segmented'
            ? {
                gridTemplateColumns:
                  orientation === 'horizontal' ? `repeat(${options.length}, minmax(0, 1fr))` : 'minmax(0, 1fr)',
              }
            : undefined
        }
      >
        {options.map((option) => (
          <label
            className={cn(
              variant === 'segmented'
                ? 'min-w-0 cursor-pointer'
                : 'flex min-w-0 cursor-pointer gap-3 text-sm text-text has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-60',
              variant === 'list'
                ? 'min-h-10 items-start py-2'
                : variant === 'cards'
                  ? 'items-start rounded-control border border-border bg-surface p-3 hover:bg-hover'
                  : undefined,
              variant === 'cards' && 'has-[:checked]:border-accent has-[:checked]:bg-selected',
            )}
            key={option.value}
          >
            <input
              checked={value === option.value}
              className={cn(
                variant === 'segmented'
                  ? 'peer sr-only'
                  : 'mt-1 size-4 shrink-0 accent-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus',
              )}
              disabled={disabled || option.disabled}
              name={name ?? generatedName}
              onChange={() => onValueChange(option.value)}
              type="radio"
              value={option.value}
            />
            {variant === 'segmented' ? (
              <span className="openchat-segment-label flex min-h-9 w-full items-center justify-center rounded-control px-2 text-center text-sm text-secondary-text hover:bg-hover hover:text-text peer-checked:bg-selected peer-checked:text-text peer-focus-visible:ring-2 peer-focus-visible:ring-focus peer-disabled:cursor-not-allowed peer-disabled:opacity-50">
                {option.label}
              </span>
            ) : (
              <span className="min-w-0 flex-1">
                <span className="block">{option.label}</span>
                {option.description ? (
                  <span className="mt-1 block text-xs text-secondary-text">{option.description}</span>
                ) : null}
              </span>
            )}
          </label>
        ))}
      </div>
    </fieldset>
  );
}
