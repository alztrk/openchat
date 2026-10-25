import * as SelectPrimitive from '@radix-ui/react-select';
import { Check, ChevronDown, ChevronUp } from 'lucide-react';
import { type ReactNode, useId } from 'react';

import { cn } from '@/shared/utils/cn';

/** A selectable value with optional leading icon and disabled state. */
export interface SelectOption {
  /** Stable option value returned through `onChange`. */
  value: string;
  /** Visible option label. */
  label: string;
  /** Optional decorative icon rendered before the label. */
  icon?: ReactNode;
  /** Prevents pointer and keyboard selection. */
  disabled?: boolean;
}

/** Controlled props for OpenChat's accessible select. */
export interface SelectProps {
  /** Accessible name overriding the visible label for the trigger. */
  'aria-label'?: string;
  /** Additional trigger classes. */
  className?: string;
  /** Prevents opening and selection. */
  disabled?: boolean;
  /** Optional stable trigger ID. */
  id?: string;
  /** Optional icon shown before the selected value in the trigger. */
  leadingIcon?: ReactNode;
  /** Visible field label. */
  label: string;
  /** Form field name. */
  name?: string;
  /** Called with the selected value using a select-like event shape. */
  onChange?: (event: { target: { value: string }; currentTarget: { value: string } }) => void;
  /** Available options in display order. */
  options: readonly SelectOption[];
  /** Optional text shown when no option is selected. */
  placeholder?: string;
  /** Requires a selected value. */
  required?: boolean;
  /** Currently selected value. */
  value?: string;
  /** Hides the visual label while keeping it available to assistive technology. */
  visuallyHiddenLabel?: boolean;
}

/** Renders the shared select control with managed focus and keyboard behavior. */
export function Select({
  'aria-label': ariaLabel,
  className,
  disabled = false,
  id,
  leadingIcon,
  label,
  name,
  onChange,
  options,
  placeholder,
  required,
  value,
  visuallyHiddenLabel = false,
}: SelectProps) {
  const generatedId = useId();
  const selectId = id ?? generatedId;
  const labelId = `${selectId}-label`;
  const selectedOption = options.find((option) => option.value === value);

  return (
    <div className="grid min-w-0 gap-2">
      <label
        className={visuallyHiddenLabel ? 'sr-only' : 'text-sm font-medium text-text'}
        htmlFor={selectId}
        id={labelId}
      >
        {label}
      </label>
      <SelectPrimitive.Root
        disabled={disabled}
        name={name}
        onValueChange={(nextValue) => onChange?.({ target: { value: nextValue }, currentTarget: { value: nextValue } })}
        required={required}
        value={value}
      >
        <SelectPrimitive.Trigger
          aria-label={ariaLabel}
          aria-labelledby={ariaLabel ? undefined : labelId}
          className={cn('openchat-dropdown-trigger flex w-full items-center justify-between gap-2', className)}
          id={selectId}
        >
          {(leadingIcon ?? selectedOption?.icon) ? (
            <span aria-hidden="true" className="flex shrink-0 items-center text-secondary-icon">
              {leadingIcon ?? selectedOption?.icon}
            </span>
          ) : null}
          <SelectPrimitive.Value
            className="min-w-0 flex-1 truncate"
            placeholder={placeholder ?? (required ? undefined : options[0]?.label)}
          >
            {selectedOption ? <span className="block truncate">{selectedOption.label}</span> : undefined}
          </SelectPrimitive.Value>
          <SelectPrimitive.Icon aria-hidden="true" className="shrink-0 text-secondary-icon">
            <ChevronDown size={15} />
          </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>
        <SelectPrimitive.Portal>
          <SelectPrimitive.Content
            className="openchat-dropdown-surface openchat-select-content"
            position="popper"
            sideOffset={6}
          >
            <SelectPrimitive.ScrollUpButton className="openchat-select-scroll-button" aria-hidden="true">
              <ChevronUp size={15} />
            </SelectPrimitive.ScrollUpButton>
            <SelectPrimitive.Viewport>
              {options.map((option) => (
                <SelectPrimitive.Item
                  className="openchat-select-item"
                  disabled={option.disabled}
                  key={option.value}
                  textValue={option.label}
                  value={option.value}
                >
                  {option.icon ? (
                    <span aria-hidden="true" className="openchat-select-item-icon">
                      {option.icon}
                    </span>
                  ) : null}
                  <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="openchat-select-item-indicator">
                    <Check aria-hidden="true" size={14} />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.Viewport>
            <SelectPrimitive.ScrollDownButton className="openchat-select-scroll-button" aria-hidden="true">
              <ChevronDown size={15} />
            </SelectPrimitive.ScrollDownButton>
          </SelectPrimitive.Content>
        </SelectPrimitive.Portal>
      </SelectPrimitive.Root>
    </div>
  );
}
