import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@shared/components';
import { LocalServiceError, localServiceErrorDetails } from '@shared/lib/platform/local-service';

interface ApplicationErrorBoundaryProps {
  children: ReactNode;
}

interface ApplicationErrorBoundaryState {
  error: LocalServiceError | null;
}

function normalizeRenderError(error: unknown): LocalServiceError {
  if (error instanceof LocalServiceError) {
    return new LocalServiceError(
      error.code,
      'The application interface could not be displayed.',
      error.retryable,
      null,
      {
        category: error.category,
        diagnosticCode: error.diagnosticCode,
        correlationId: error.correlationId,
        retryAfterMs: error.retryAfterMs,
      },
    );
  }
  return new LocalServiceError(
    'frontend_render_failed',
    'The application interface could not be displayed.',
    false,
  );
}

export class ApplicationErrorBoundary extends Component<
  ApplicationErrorBoundaryProps,
  ApplicationErrorBoundaryState
> {
  override state: ApplicationErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ApplicationErrorBoundaryState {
    return { error: normalizeRenderError(error) };
  }

  override componentDidCatch(error: Error, _errorInfo: ErrorInfo): void {
    globalThis.reportError(normalizeRenderError(error));
  }

  override render(): ReactNode {
    return this.state.error ? <ApplicationErrorFallback error={this.state.error} /> : this.props.children;
  }
}

function ApplicationErrorFallback({ error }: { error: LocalServiceError }) {
  const { t } = useTranslation();
  const details = localServiceErrorDetails(error);

  return (
    <main className="flex min-h-full items-center justify-center p-6">
      <section
        aria-labelledby="application-error-title"
        className="grid w-full max-w-xl gap-4 rounded-panel border border-border bg-surface p-6 text-text shadow-lg"
        role="alert"
      >
        <div className="grid gap-2">
          <h1 className="text-lg font-semibold" id="application-error-title">
            {t('applicationRenderErrorTitle')}
          </h1>
          <p className="text-sm text-secondary-text">{t('applicationRenderErrorDescription')}</p>
        </div>
        <details className="text-sm">
          <summary className="cursor-pointer text-secondary-text">{t('startupErrorDetails')}</summary>
          <pre className="mt-2 whitespace-pre-wrap break-words rounded-control bg-background p-3 font-mono text-xs text-secondary-text">
            {details}
          </pre>
        </details>
        <div>
          <Button onClick={() => window.location.reload()} variant="primary">
            {t('applicationRenderErrorReload')}
          </Button>
        </div>
      </section>
    </main>
  );
}
