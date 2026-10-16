import { Brain } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { MessageMarkdown } from './MessageMarkdown';
import type { ReasoningSummary as ReasoningSummaryData } from './messageData';

interface ReasoningSummaryProps {
  summary: ReasoningSummaryData;
}

export function ReasoningSummary({ summary }: ReasoningSummaryProps) {
  const { t } = useTranslation();
  const title = summary.elapsedMicroseconds === null
    ? t('reasoningSummary')
    : t('reasoningSummaryWithDuration', { duration: formatDuration(summary.elapsedMicroseconds) });
  return (
    <details className="openchat-native-disclosure rounded-card border border-border bg-surface" open={!summary.isComplete}>
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm text-secondary-text [&::-webkit-details-marker]:hidden">
        <Brain aria-hidden="true" size={16} />
        <span>{title}</span>
      </summary>
      <div className="border-t border-border px-3 pb-3 pt-2 text-sm text-secondary-text">
        <MessageMarkdown content={summary.content} citationSources={[]} />
      </div>
    </details>
  );
}

function formatDuration(elapsedMicroseconds: number): string {
  const seconds = Math.floor(elapsedMicroseconds / 1_000_000);
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
