import { ChevronDown, LoaderCircle, Wrench } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

interface ToolActivityGroupProps {
  activityCount: number;
  children: ReactNode;
  elapsedTimeLabel: string | null;
  isOpen: boolean;
  isResponseInProgress: boolean;
  onOpenChange(open: boolean): void;
}

export function ToolActivityGroup({
  activityCount,
  children,
  elapsedTimeLabel,
  isOpen,
  isResponseInProgress,
  onOpenChange,
}: ToolActivityGroupProps) {
  const { t } = useTranslation();
  if (activityCount === 0) return null;

  return (
    <details
      className="openchat-tool-accordion openchat-native-disclosure mb-2 max-w-assistant-message overflow-hidden rounded-card border border-border bg-surface"
      onToggle={(event) => onOpenChange(event.currentTarget.open)}
      open={isOpen}
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm text-text [&::-webkit-details-marker]:hidden">
        {isResponseInProgress ? (
          <LoaderCircle
            aria-hidden="true"
            className="shrink-0 animate-spin text-accent-icon motion-reduce:animate-none"
            size={16}
          />
        ) : (
          <Wrench aria-hidden="true" className="shrink-0 text-accent-icon" size={16} />
        )}
        <span aria-live="polite" className="font-semibold">
          {isResponseInProgress ? t('toolActivityWorking') : t('toolActivityTitle')}
        </span>
        {elapsedTimeLabel !== null ? (
          <span className="ml-auto shrink-0 text-xs text-secondary-text">{elapsedTimeLabel}</span>
        ) : null}
        <ChevronDown
          aria-hidden="true"
          className={`shrink-0 text-secondary-icon transition-transform motion-reduce:transition-none ${isOpen ? 'rotate-180' : ''}`}
          size={15}
        />
      </summary>
      <div className="border-t border-border">{children}</div>
    </details>
  );
}
