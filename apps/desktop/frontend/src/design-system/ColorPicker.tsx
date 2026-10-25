import { useId } from 'react';

interface ColorPickerProps {
  disabled?: boolean;
  label: string;
  onChange(value: string): void;
  value: string;
}

/** Accessible color swatch with the native platform color chooser. */
export function ColorPicker({ disabled = false, label, onChange, value }: ColorPickerProps) {
  const id = useId();

  return (
    <label className="grid min-w-0 gap-2 text-sm font-medium text-text" htmlFor={id}>
      {label}
      <span className="flex min-h-10 min-w-0 items-center gap-2 rounded-control border border-control-border bg-surface px-2">
        <input
          aria-label={label}
          className="size-7 shrink-0 cursor-pointer rounded border-0 bg-transparent p-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 disabled:cursor-not-allowed"
          disabled={disabled}
          id={id}
          onChange={(event) => onChange(event.target.value)}
          type="color"
          value={value}
        />
        <span aria-hidden="true" className="font-mono text-sm font-normal text-secondary-text">
          {value.toUpperCase()}
        </span>
      </span>
    </label>
  );
}
