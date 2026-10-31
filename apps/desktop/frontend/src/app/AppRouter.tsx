import { lazy, Suspense, useCallback, useRef, useState } from 'react';
import type { AppSurface } from '@widgets/navigation-rail';
import type { AppearancePreferences } from '@features/app-settings/data/appearancePreferences';
import type { ThemePreference } from '@features/app-settings/components/GeneralSettingsSection';
import { readInitialSurface, writeLastSurface } from '@features/app-settings/data/appPreferences';
import { invoke } from '@shared/lib/platform/tauri';
// eslint-disable-next-line @typescript-eslint/no-unused-vars -- reserved for future surface routing needs
void invoke;

// Feature imports — wired by consumer in full App.tsx
const ConversationPane = lazy(() => import('@features/chat/components/ConversationPane').then((m) => ({ default: m.ConversationPane })));
const ConversationSidebar = lazy(() => import('@features/chat/components/ConversationSidebar').then((m) => ({ default: m.ConversationSidebar })));
const LibraryPage = lazy(() => import('@features/model-catalog/components/LibraryPage').then((m) => ({ default: m.LibraryPage })));
const DesignPage = lazy(() => import('@features/design-workspace/components/DesignPage').then((m) => ({ default: m.DesignPage })));
const SecurityScanPage = lazy(() => import('@features/security-scans/components/SecurityScanPage').then((m) => ({ default: m.SecurityScanPage })));
const SettingsPage = lazy(() => import('@features/app-settings/components/SettingsPage').then((m) => ({ default: m.SettingsPage })));
// eslint-disable-next-line @typescript-eslint/no-unused-vars -- reserved import, wired by consumer in App.tsx
void ConversationSidebar;

export interface SurfaceHistory {
  surfaces: AppSurface[];
  index: number;
}

export function useAppRouter() {
  const [surfaceHistory, setSurfaceHistory] = useState<SurfaceHistory>({
    surfaces: [readInitialSurface()],
    index: 0,
  });
  const surfaceHeadingRef = useRef<HTMLElement>(null);
  const activeSurface = surfaceHistory.surfaces[surfaceHistory.index] ?? 'chats';

  const navigateSurface = useCallback((surface: AppSurface) => {
    try { writeLastSurface(surface); } catch { /* persisted */ }
    setSurfaceHistory((cur) => {
      if (cur.surfaces[cur.index] === surface) return cur;
      const surfaces = [...cur.surfaces.slice(0, cur.index + 1), surface];
      if (surfaces.length > 50) surfaces.shift();
      return { surfaces, index: surfaces.length - 1 };
    });
  }, []);

  const moveInHistory = useCallback((offset: number) => {
    setSurfaceHistory((cur) => {
      const next = cur.index + offset;
      return cur.surfaces[next] ? { ...cur, index: next } : cur;
    });
  }, []);

  return { activeSurface, surfaceHistory, surfaceHeadingRef, navigateSurface, moveInHistory } as const;
}

export function AppRouter({
  theme = 'system',
  // biome-ignore lint/nursery/noUnsafeTypeAssertion: default value for optional prop; AppearancePreferences has no exported constructor
  appearance = { appScale: '', appDensity: '', appFont: '', conversationTextSize: 'normal', conversationWidth: 'medium' } as unknown as AppearancePreferences,
  onOpenSettings = () => { /* noop */ },
  onUsePrompt = () => { /* noop */ },
}: { theme?: ThemePreference; appearance?: AppearancePreferences; onOpenSettings?: () => void; onUsePrompt?: (draft: string) => void }) {
  const { activeSurface, surfaceHeadingRef } = useAppRouter();
  return (
    <main aria-label={activeSurface} className="app-main-surface relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden" key={activeSurface} ref={surfaceHeadingRef} tabIndex={-1}>
      {activeSurface === 'chats' && (
        <div className="flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden">
          <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
            <Suspense fallback={null}>
              <ConversationPane isLargeText={appearance.conversationTextSize === 'large'} notificationFeedbackKey={null} onDismissNotificationFeedback={() => { /* noop */ }} onOpenSettings={onOpenSettings} onQuestionFocusHandled={() => { /* noop */ }} onRequestTemporaryExit={() => { /* noop */ }} questionFocusTarget={null} />
            </Suspense>
          </div>
        </div>
      )}
      {activeSurface === 'models' && (
        <Suspense fallback={null}><LibraryPage onUsePrompt={onUsePrompt} /></Suspense>
      )}
      {activeSurface === 'design' && (
        <Suspense fallback={null}><DesignPage onOpenSettings={onOpenSettings} /></Suspense>
      )}
      {activeSurface === 'security' && (
        <Suspense fallback={null}><SecurityScanPage /></Suspense>
      )}
      {activeSurface !== 'chats' && activeSurface !== 'models' && activeSurface !== 'design' && activeSurface !== 'security' && (
        <Suspense fallback={null}>
          <SettingsPage appearance={appearance} onAppearanceChange={() => { /* noop */ }} onOpenConversation={() => { /* noop */ }} onThemeChange={() => { /* noop */ }} theme={theme} />
        </Suspense>
      )}
    </main>
  );
}
