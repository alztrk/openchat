import { Blocks, BookmarkCheck, Cpu, Files } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import { McpCatalogTab } from './McpCatalogTab';
import { ModelCatalogTab } from './ModelCatalogTab';
import { SavedResponsesTab } from './SavedResponsesTab';
import { AttachmentLibraryTab } from './AttachmentLibraryTab';

type LibraryPageTab = 'models' | 'mcp' | 'outputs' | 'files';

export function LibraryPage() {
  const { t } = useTranslation();
  const [page, setPage] = useState<LibraryPageTab>('models');
  const [visitedPages, setVisitedPages] = useState<ReadonlySet<LibraryPageTab>>(() => new Set(['models']));
  const headingRef = useRef<HTMLHeadingElement>(null);

  const selectPage = (nextPage: LibraryPageTab) => {
    setPage(nextPage);
    setVisitedPages((current) => (current.has(nextPage) ? current : new Set([...current, nextPage])));
  };

  useEffect(() => {
    headingRef.current?.focus({ preventScroll: true });
  }, []);

  return (
    <div className="library-workspace flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-background">
      <header className="library-page-header flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-border bg-navigation px-5 py-3 sm:px-8">
        <h1
          aria-live="polite"
          className="min-w-0 text-xl font-semibold leading-7 text-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus focus-visible:ring-offset-2 focus-visible:ring-offset-background"
          ref={headingRef}
          tabIndex={-1}
        >
          {t('modelLibrary')}
        </h1>
        <nav
          aria-label={t('modelLibrary')}
          className="library-page-nav flex w-fit max-w-full gap-1 overflow-x-auto rounded-control border border-border bg-background p-1"
        >
          <Button
            aria-current={page === 'models' ? 'page' : undefined}
            className={page === 'models' ? 'library-page-nav-item is-active' : 'library-page-nav-item'}
            onClick={() => selectPage('models')}
            variant="ghost"
          >
            <Cpu aria-hidden="true" size={16} />
            {t('libraryTabModels')}
          </Button>
          <Button
            aria-current={page === 'outputs' ? 'page' : undefined}
            className={page === 'outputs' ? 'library-page-nav-item is-active' : 'library-page-nav-item'}
            onClick={() => selectPage('outputs')}
            variant="ghost"
          >
            <BookmarkCheck aria-hidden="true" size={16} />
            {t('outputs')}
          </Button>
          <Button
            aria-current={page === 'files' ? 'page' : undefined}
            className={page === 'files' ? 'library-page-nav-item is-active' : 'library-page-nav-item'}
            onClick={() => selectPage('files')}
            variant="ghost"
          >
            <Files aria-hidden="true" size={16} />
            {t('attachmentLibraryTitle')}
          </Button>
          <Button
            aria-current={page === 'mcp' ? 'page' : undefined}
            className={page === 'mcp' ? 'library-page-nav-item is-active' : 'library-page-nav-item'}
            onClick={() => selectPage('mcp')}
            variant="ghost"
          >
            <Blocks aria-hidden="true" size={16} />
            {t('libraryTabMcp')}
          </Button>
        </nav>
      </header>

      {visitedPages.has('models') ? (
        <div
          aria-hidden={page !== 'models'}
          className={page === 'models' ? 'library-page-panel flex min-h-0 min-w-0 flex-1 flex-col' : 'hidden'}
        >
          <ModelCatalogTab />
        </div>
      ) : null}
      {visitedPages.has('mcp') ? (
        <div
          aria-hidden={page !== 'mcp'}
          className={page === 'mcp' ? 'library-page-panel min-h-0 min-w-0 flex-1 overflow-y-auto' : 'hidden'}
        >
          <McpCatalogTab />
        </div>
      ) : null}
      {visitedPages.has('outputs') ? (
        <div
          aria-hidden={page !== 'outputs'}
          className={page === 'outputs' ? 'library-page-panel min-h-0 min-w-0 flex-1 overflow-y-auto' : 'hidden'}
        >
          <SavedResponsesTab />
        </div>
      ) : null}
      {visitedPages.has('files') ? (
        <div
          aria-hidden={page !== 'files'}
          className={page === 'files' ? 'library-page-panel min-h-0 min-w-0 flex-1 overflow-y-auto' : 'hidden'}
        >
          <AttachmentLibraryTab />
        </div>
      ) : null}
    </div>
  );
}
