import type { ReactNode } from 'react';
import { useId } from 'react';

interface SettingsPreferenceRowProps {
  children: ReactNode | ((descriptionId: string) => ReactNode);
  description: string;
  id?: string;
  label: string;
}

/** Lays out a setting label and description beside its control and links them accessibly. */
export function SettingsPreferenceRow({ children, description, id, label }: SettingsPreferenceRowProps) {
  const descriptionId = useId();

  return (
    <div className="@container" id={id} tabIndex={id ? -1 : undefined}>
      <div className="settings-preference-row p-4">
        <div>
          <h2 className="font-medium text-text">{label}</h2>
          <p className="mt-1 text-sm text-secondary-text" id={descriptionId}>
            {description}
          </p>
        </div>
        <div className="min-w-0">{typeof children === 'function' ? children(descriptionId) : children}</div>
      </div>
    </div>
  );
}
