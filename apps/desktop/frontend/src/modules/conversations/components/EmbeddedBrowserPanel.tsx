import { ArrowLeft, ArrowRight, Globe, RotateCw } from 'lucide-react';
import { type FormEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';

export interface EmbeddedBrowserPanelProps {
  initialUrl?: string;
}

function normalizeInputUrl(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const withScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(trimmed)
    ? trimmed
    : trimmed.startsWith('localhost') || trimmed.startsWith('127.0.0.1')
      ? `http://${trimmed}`
      : `https://${trimmed}`;
  try {
    const parsed = new URL(withScheme);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return null;
    if (parsed.username || parsed.password) return null;
    return parsed.toString();
  } catch {
    return null;
  }
}

export function EmbeddedBrowserPanel({ initialUrl }: EmbeddedBrowserPanelProps) {
  const { t } = useTranslation();
  const [history, setHistory] = useState<string[]>(() => {
    const normalized = initialUrl ? normalizeInputUrl(initialUrl) : null;
    return normalized ? [normalized] : ['https://example.com'];
  });
  const [historyIndex, setHistoryIndex] = useState(0);
  const currentUrl = history[historyIndex] ?? 'https://example.com';
  const [addressInput, setAddressInput] = useState(currentUrl);
  const [reloadNonce, setReloadNonce] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    setAddressInput(currentUrl);
  }, [currentUrl]);

  const navigateTo = (raw: string) => {
    const normalized = normalizeInputUrl(raw);
    if (!normalized) {
      setErrorMessage(t('browserPanelInvalidUrl'));
      return;
    }
    setErrorMessage(null);
    setAddressInput(normalized);
    setHistory((prev) => {
      const sliced = prev.slice(0, historyIndex + 1);
      if (sliced[sliced.length - 1] === normalized) return sliced;
      return [...sliced, normalized];
    });
    setHistoryIndex((prev) => {
      const current = history[prev];
      return current === normalized ? prev : prev + 1;
    });
  };

  const handleAddressSubmit = (event: FormEvent) => {
    event.preventDefault();
    navigateTo(addressInput);
  };

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col bg-surface">
      <div className="border-b border-border px-3 py-2">
        <form className="flex items-center gap-1.5" onSubmit={handleAddressSubmit}>
          <Button
            aria-label={t('browserPanelBack')}
            className="size-8 shrink-0"
            disabled={historyIndex <= 0}
            onClick={() => setHistoryIndex((idx) => Math.max(0, idx - 1))}
            size="icon"
            type="button"
            variant="ghost"
          >
            <ArrowLeft aria-hidden="true" size={14} />
          </Button>
          <Button
            aria-label={t('browserPanelForward')}
            className="size-8 shrink-0"
            disabled={historyIndex >= history.length - 1}
            onClick={() => setHistoryIndex((idx) => Math.min(history.length - 1, idx + 1))}
            size="icon"
            type="button"
            variant="ghost"
          >
            <ArrowRight aria-hidden="true" size={14} />
          </Button>
          <Button
            aria-label={t('browserPanelReload')}
            className="size-8 shrink-0"
            onClick={() => setReloadNonce((n) => n + 1)}
            size="icon"
            type="button"
            variant="ghost"
          >
            <RotateCw aria-hidden="true" size={14} />
          </Button>
          <div className="relative flex min-w-0 flex-1 items-center">
            <Globe aria-hidden="true" className="pointer-events-none absolute left-2.5 text-secondary-icon" size={13} />
            <input
              aria-label={t('browserPanelAddressLabel')}
              className="h-8 w-full rounded-control border border-border bg-composer pl-7 pr-2.5 text-xs text-text focus:outline-none"
              onChange={(event) => setAddressInput(event.currentTarget.value)}
              placeholder={t('browserPanelUrlPlaceholder')}
              type="text"
              value={addressInput}
            />
          </div>
        </form>
      </div>

      {errorMessage ? (
        <p className="mx-3 mt-2 rounded-control bg-destructive/10 px-2.5 py-2 text-xs text-destructive" role="alert">
          {errorMessage}
        </p>
      ) : null}

      <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
        <iframe
          className="h-full w-full flex-1 border-0 bg-white"
          key={`${currentUrl}-${reloadNonce}`}
          referrerPolicy="no-referrer"
          sandbox="allow-scripts allow-forms allow-same-origin"
          src={currentUrl}
          title={currentUrl}
        />
      </div>
    </div>
  );
}
