import { useId } from 'react';

/** One mutually exclusive choice in a segmented control. */
export interface SegmentedOption {
  /** Stable value emitted when this option is selected. */
  value: string;
  /** Visible and accessible option label. */
  label: string;
}

/** Controlled props for a labeled, mutually exclusive segmented choice. */
export interface SegmentedControlProps {
  /** Accessible group name. */
  label: string;
  /** Available options in display order. */
  options: readonly SegmentedOption[];
  /** Value that is currently selected. */
  value: string;
  /** Called with the selected option's value. */
  onValueChange(value: string): void;
  /** Disables the entire group. */
  disabled?: boolean;
}

/** Renders a controlled radio group with the shared segmented-control treatment. */
export function SegmentedControl({ disabled = false, label, onValueChange, options, value }: SegmentedControlProps) {
  const id = useId();

  return (
    <fieldset aria-label={label} className="min-w-0" disabled={disabled}>
      <legend className="sr-only">{label}</legend>
      <div
        className="grid w-full gap-1 rounded-button border border-border bg-raised p-1"
        style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
      >
        {options.map((option) => (
          <label className="min-w-0 cursor-pointer" key={option.value}>
            <input
              checked={value === option.value}
              className="peer sr-only"
              id={`${id}-${option.value}`}
              name={id}
              onChange={() => onValueChange(option.value)}
              type="radio"
              value={option.value}
            />
            <span className="openchat-segment-label flex min-h-9 items-center justify-center rounded-control px-2 text-center text-sm text-secondary-text hover:bg-hover hover:text-text peer-checked:bg-selected peer-checked:text-text peer-focus-visible:ring-2 peer-focus-visible:ring-focus peer-disabled:cursor-not-allowed peer-disabled:opacity-50">
              {option.label}
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
