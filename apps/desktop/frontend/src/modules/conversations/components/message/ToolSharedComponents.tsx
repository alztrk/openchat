import { AlertCircle, CircleAlert, Copy, File, Info, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { formatToolPath, preview, safeAttributionNodes } from './toolResultUtils';

export function PathHeading({ path }: { path: string }) {
  const { t } = useTranslation();
  return (
    <p className="flex min-w-0 items-center gap-2 text-xs text-secondary-text">
      <File aria-hidden="true" className="shrink-0" size={14} />
      <span className="truncate" title={path}>
        {formatToolPath(path, t)}
      </span>
    </p>
  );
}

export function CopyTextButton({ content, label }: { content: string; label: string }) {
  const { t } = useTranslation();
  const [copyFailed, setCopyFailed] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopyFailed(false);
    } catch {
      setCopyFailed(true);
    }
  };
  return (
    <span className="grid shrink-0 justify-items-center">
      <Button
        aria-label={label}
        className="size-11"
        title={label}
        onClick={() => void copy()}
        size="icon"
        variant="ghost"
      >
        <Copy aria-hidden="true" size={14} />
      </Button>
      {copyFailed ? (
        <span className="text-xs text-destructive" role="alert">
          {t('toolCopyFailed')}
        </span>
      ) : null}
    </span>
  );
}

export function SafeWebAttribution({ html }: { html: string }) {
  const { t } = useTranslation();
  const nodes = safeAttributionNodes(html);
  if (nodes.length === 0) return null;
  return (
    <section aria-label={t('toolSourceDetails')} className="text-xs leading-5 text-secondary-text">
      {nodes}
    </section>
  );
}

export function PreviewBlock({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <p className="mb-1 text-xs font-medium text-secondary-text">{label}</p>
      <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2 font-code text-xs text-text">
        {preview(value)}
      </pre>
    </div>
  );
}

export function DiffBlock({ label, value, truncated }: { label: string; value: string; truncated: boolean }) {
  const { t } = useTranslation();
  return (
    <section aria-label={label} className="grid gap-1">
      <h4 className="text-xs font-semibold text-secondary-text">{label}</h4>
      <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded-control border border-border bg-surface p-2 font-code text-xs leading-5 text-text">
        {value}
      </pre>
      {truncated ? <ResultFootnote message={t('toolOperationTruncated')} /> : null}
    </section>
  );
}

export function MetaChip({ label, value }: { label: string; value?: string }) {
  return (
    <span className="rounded-control border border-border bg-surface px-2 py-1 text-secondary-text">
      {value ? (
        <>
          <span>{label}: </span>
          <span className="font-medium text-text">{value}</span>
        </>
      ) : (
        label
      )}
    </span>
  );
}

export function ResultFootnote({ message }: { message: string }) {
  return (
    <p className="flex items-start gap-1.5 text-xs leading-5 text-secondary-text">
      <Info aria-hidden="true" className="mt-0.5 shrink-0" size={13} />
      <span>{message}</span>
    </p>
  );
}

export function ToolResultNotice({
  message,
  path,
  isError = false,
  isWorking = false,
}: {
  message: string;
  path?: string | null;
  isError?: boolean;
  isWorking?: boolean;
}) {
  const { t } = useTranslation();
  const Icon = isWorking ? LoaderCircle : isError ? CircleAlert : AlertCircle;
  return (
    <div
      className={`flex items-start gap-2 rounded-control px-2.5 py-2 text-xs leading-5 ${isError ? 'border border-destructive/30 bg-destructive/10 text-destructive' : 'bg-surface text-secondary-text'}`}
    >
      {isWorking || isError ? (
        <Icon aria-hidden="true" className={`mt-0.5 shrink-0 ${isWorking ? 'animate-spin' : ''}`} size={15} />
      ) : null}
      <div className="min-w-0 flex-1">
        <span
          aria-live={isWorking || isError ? 'polite' : undefined}
          role={isWorking || isError ? 'status' : undefined}
        >
          {message}
        </span>
        {path ? (
          <p className="mt-1 break-all font-code text-secondary-text" title={path}>
            {formatToolPath(path, t)}
          </p>
        ) : null}
      </div>
    </div>
  );
}
