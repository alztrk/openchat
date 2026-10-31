import { RadioGroup, type RadioOption } from './RadioGroup';

/** One mutually exclusive choice in a segmented control. */
export interface SegmentedOption {
  value: string;
  label: string;
  disabled?: boolean;
}

/** Controlled props for a labeled, mutually exclusive segmented choice. */
export interface SegmentedControlProps {
  label: string;
  'aria-describedby'?: string;
  options: readonly SegmentedOption[];
  value: string;
  onValueChange(value: string): void;
  disabled?: boolean;
  orientation?: 'horizontal' | 'vertical';
}

/** Renders the segmented presentation through the shared native radio-group primitive. */
export function SegmentedControl({
  'aria-describedby': describedBy,
  disabled,
  label,
  onValueChange,
  options,
  orientation = 'horizontal',
  value,
}: SegmentedControlProps) {
  const radioOptions: RadioOption[] = options.map((option) => ({
    value: option.value,
    label: option.label,
    disabled: option.disabled,
  }));

  return (
    <RadioGroup
      aria-describedby={describedBy}
      disabled={disabled}
      label={label}
      onValueChange={onValueChange}
      options={radioOptions}
      orientation={orientation}
      value={value}
      variant="segmented"
    />
  );
}
