import { Github } from 'lucide-react';
import { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Tabs, TabsContent } from '@/design-system/tabs';
import { ConversationPane } from '@/modules/conversations/components/ConversationPane';
import { ConversationSearchDialog } from '@/modules/conversations/components/ConversationSearchDialog';
import { TemporaryChatExitDialog } from '@/modules/conversations/components/TemporaryChatExitDialog';
import { parseAgentQuestionGroup } from '@/modules/conversations/data/agentQuestions';
import { getMessages } from '@/modules/conversations/data/chatStorage';
import type { HistorySearchResult } from '@/modules/conversations/data/historySearch';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import {
  resetNotificationWindowFocus,
  setNotificationWindowFocus,
} from '@/modules/notifications/data/notificationRuntime';
import type { ThemePreference } from '@/modules/settings/components/GeneralSettingsSection';
import { SettingsPage } from '@/modules/settings/components/SettingsPage';
import {
  type AppearancePreferences,
  getAppearanceCssValues,
  readAppearancePreferences,
} from '@/modules/settings/data/appearancePreferences';
import { localServiceClient } from '@/platform/local-service';
import { getCurrentWindow, getVersion, invoke, isTauri, listen } from '@/platform/tauri';
import { BrandMark } from '@/shell/BrandMark';
import { ConversationActionsHeader } from '@/shell/ConversationActionsHeader';
import { type AppSurface, NavigationRail } from '@/shell/NavigationRail';
import { TitleBar } from '@/shell/TitleBar';

const LibraryPage = lazy(() =>
  import('@/modules/model-library/components/LibraryPage').then((module) => ({ default: module.LibraryPage })),
);

interface SurfaceHistory {
  surfaces: AppSurface[];
  index: number;
}

type NotificationFeedbackKey =
  | 'assistantResponseNotificationReplyEmpty'
  | 'assistantResponseNotificationReplyTooLong'
  | 'assistantResponseNotificationReplyUnavailable'
  | 'assistantResponseNotificationReplyNotSent'
  | 'userQuestionUnavailable'
  | 'notificationDeliveryFailed'
  | 'notificationTestFailed';

function updateThemeWithTransition(update: () => void, enabled: boolean): void {
  if (
    !enabled ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches ||
    typeof document.startViewTransition !== 'function'
  ) {
    update();
    return;
  }

  document.startViewTransition(() => flushSync(update));
}

interface NotificationActivation {
  kind: 'openConversation' | 'openQuestion' | 'replyToAssistant';
  conversationId: string;
  groupId?: string;
  assistantMessageId?: string;
  replyText?: string | null;
  replyInputTooLong?: boolean;
}

function readThemePreference(): ThemePreference {
  try {
    const preference = localStorage.getItem('openchat.theme');
    if (preference === 'system' || preference === 'light' || preference === 'dark') {
      return preference;
    }
  } catch {
    return 'system';
  }
  return 'system';
}

export function App() {
  const { t } = useTranslation();
  const [surfaceHistory, setSurfaceHistory] = useState<SurfaceHistory>({
    surfaces: ['chats'],
    index: 0,
  });
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const activeSurface = surfaceHistory.surfaces[surfaceHistory.index] ?? 'chats';
  const [theme, setTheme] = useState<ThemePreference>(readThemePreference);
  const [appearance, setAppearance] = useState<AppearancePreferences>(readAppearancePreferences);
  const [notificationFeedback, setNotificationFeedback] = useState<NotificationFeedbackKey | null>(null);
  const [questionFocusTarget, setQuestionFocusTarget] = useState<{ conversationId: string; groupId: string } | null>(
    null,
  );
  const [temporaryExitIntent, setTemporaryExitIntent] = useState<(() => void | Promise<void>) | null>(null);
  const [isSavingTemporaryChat, setIsSavingTemporaryChat] = useState(false);
  const [historySearchOpen, setHistorySearchOpen] = useState(false);
  const [historySearchInitialQuery, setHistorySearchInitialQuery] = useState('');
  const [systemIsDark, setSystemIsDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  const surfaceHeadingRef = useRef<HTMLElement>(null);
  const lastInteractionMethod = useRef<'keyboard' | 'pointer'>('pointer');
  const initializeChat = useChatStore((state) => state.initialize);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const selectConversation = useChatStore((state) => state.selectConversation);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const activeChatTab = temporaryChat !== null ? 'temporary' : (activeConversationId ?? 'new');
  const temporaryChatMessageCount = useChatStore((state) => state.messages.length);
  const temporaryChatErrorCode = useChatStore((state) => state.errorCode);
  const isDark = theme === 'dark' || (theme === 'system' && systemIsDark);

  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    void getVersion()
      .then((version) => {
        if (active) setAppVersion(version);
      })
      .catch((cause: unknown) => {
        if (active)
          globalThis.reportError(new Error('The app version could not be read from the Tauri runtime.', { cause }));
      });
    return () => {
      active = false;
    };
  }, []);

  useLayoutEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
  }, [isDark]);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const values = getAppearanceCssValues(appearance);
    root.style.setProperty('--conversation-reading-width', values.conversationWidth);
    root.style.setProperty('--app-text-scale', values.textScale);
    root.style.setProperty('--app-font-family', values.fontFamily);
  }, [appearance]);

  useEffect(() => {
    if (isTauri()) void initializeChat();
  }, [initializeChat]);

  useEffect(() => {
    const openHistorySearch = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail)) return;
      if (typeof event.detail.query !== 'string') return;
      setHistorySearchInitialQuery(event.detail.query);
      setHistorySearchOpen(true);
    };
    window.addEventListener('openchat-history-search', openHistorySearch);
    return () => window.removeEventListener('openchat-history-search', openHistorySearch);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlistenFocus: (() => void) | undefined;
    const currentWindow = getCurrentWindow();
    void currentWindow
      .onFocusChanged(({ payload }) => {
        setNotificationWindowFocus(payload);
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenFocus = unlisten;
        return currentWindow.isFocused();
      })
      .then((focused) => {
        if (!disposed) setNotificationWindowFocus(focused);
      })
      .catch(() => resetNotificationWindowFocus());
    return () => {
      disposed = true;
      unlistenFocus?.();
      resetNotificationWindowFocus();
    };
  }, []);

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const updateSystemTheme = (event: MediaQueryListEvent) => {
      if (theme === 'system') {
        updateThemeWithTransition(() => setSystemIsDark(event.matches), true);
      } else {
        setSystemIsDark(event.matches);
      }
    };
    setSystemIsDark(media.matches);
    media.addEventListener('change', updateSystemTheme);
    return () => media.removeEventListener('change', updateSystemTheme);
  }, [theme]);

  const navigateSurface = useCallback((surface: AppSurface) => {
    setSurfaceHistory((current) => {
      if (current.surfaces[current.index] === surface) return current;
      const surfaces = [...current.surfaces.slice(0, current.index + 1), surface];
      if (surfaces.length > 50) surfaces.shift();
      return { surfaces, index: surfaces.length - 1 };
    });
  }, []);

  const requestTemporaryChatExit = useCallback((action: () => void | Promise<void>) => {
    const current = useChatStore.getState();
    if (!current.temporaryChat) {
      void Promise.resolve(action()).catch(() => {
        useChatStore.setState({ errorCode: 'messageHistoryLoadFailed' });
      });
      return;
    }
    if (current.isSending) {
      useChatStore.setState({ errorCode: 'temporaryChatStopBeforeLeaving' });
      return;
    }
    if (current.messages.length === 0) {
      current.discardTemporaryChat();
      void Promise.resolve(action()).catch(() => {
        useChatStore.setState({ errorCode: 'messageHistoryLoadFailed' });
      });
      return;
    }
    setTemporaryExitIntent(() => action);
  }, []);

  const openStatisticsConversation = useCallback(
    (conversationId: string) => {
      requestTemporaryChatExit(async () => {
        navigateSurface('chats');
        await useChatStore.getState().selectConversation(conversationId);
      });
    },
    [navigateSurface, requestTemporaryChatExit],
  );

  const selectHistorySearchResult = useCallback(
    (result: HistorySearchResult) => {
      setHistorySearchOpen(false);
      setHistorySearchInitialQuery('');
      requestTemporaryChatExit(async () => {
        navigateSurface('chats');
        if (result.kind === 'project' && result.projectId !== null) {
          startNewChat(result.projectId);
          return;
        }
        await selectConversation(result.conversationId, result.messageId);
      });
    },
    [navigateSurface, requestTemporaryChatExit, selectConversation, startNewChat],
  );

  const navigate = useCallback(
    (surface: AppSurface) => {
      if (surface === activeSurface) {
        surfaceHeadingRef.current?.focus();
        return;
      }
      requestTemporaryChatExit(() => {
        navigateSurface(surface);
      });
    },
    [activeSurface, navigateSurface, requestTemporaryChatExit],
  );

  const openHistorySearch = () => {
    setHistorySearchInitialQuery('');
    setHistorySearchOpen(true);
  };

  const startWorkspaceChat = () => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      startNewChat();
    });
  };

  const startWorkspaceTemporaryChat = () => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      startTemporaryChat();
    });
  };

  const selectChatTab = (value: string) => {
    if (value === 'temporary') return;
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      if (value === 'new') {
        startNewChat();
        return;
      }
      void selectConversation(value);
    });
  };

  const moveInHistory = (offset: number) => {
    requestTemporaryChatExit(() => {
      setSurfaceHistory((current) => {
        const nextIndex = current.index + offset;
        return current.surfaces[nextIndex] ? { ...current, index: nextIndex } : current;
      });
    });
  };

  const closeTemporaryChatExitDialog = (open: boolean) => {
    if (!open && !isSavingTemporaryChat) setTemporaryExitIntent(null);
  };

  const discardTemporaryChatAndContinue = () => {
    if (!temporaryExitIntent || isSavingTemporaryChat) return;
    const action = temporaryExitIntent;
    useChatStore.getState().discardTemporaryChat();
    setTemporaryExitIntent(null);
    void Promise.resolve(action()).catch(() => {
      useChatStore.setState({ errorCode: 'messageHistoryLoadFailed' });
    });
  };

  const saveTemporaryChatAndContinue = async () => {
    if (!temporaryExitIntent || isSavingTemporaryChat) return;
    setIsSavingTemporaryChat(true);
    try {
      if (!(await useChatStore.getState().saveTemporaryChat())) return;
      const action = temporaryExitIntent;
      setTemporaryExitIntent(null);
      await action();
    } catch {
      useChatStore.setState({ errorCode: 'messageHistoryLoadFailed' });
    } finally {
      setIsSavingTemporaryChat(false);
    }
  };

  const handleNotificationActivation = useCallback(
    async (activation: NotificationActivation) => {
      if (useChatStore.getState().temporaryChat) {
        requestTemporaryChatExit(() => notificationActivationHandlerRef.current(activation));
        return;
      }
      navigateSurface('chats');
      if (useChatStore.getState().serviceStatus !== 'ready') {
        await useChatStore.getState().initialize();
      }
      const chat = useChatStore.getState();
      if (chat.activeConversationId !== activation.conversationId) {
        await chat.selectConversation(activation.conversationId);
      }
      if (activation.kind === 'openQuestion' && activation.groupId) {
        setQuestionFocusTarget({
          conversationId: activation.conversationId,
          groupId: activation.groupId,
        });
        return;
      }
      if (activation.kind !== 'replyToAssistant') return;

      const reply = activation.replyText?.trim() ?? '';
      const preserveReply = () => {
        if (reply) {
          window.dispatchEvent(
            new CustomEvent('openchat-notification-reply-draft', {
              detail: { conversationId: activation.conversationId, text: reply },
            }),
          );
        }
      };
      if (activation.replyInputTooLong || reply.length > 8192 || [...reply].length > 4096) {
        setNotificationFeedback('assistantResponseNotificationReplyTooLong');
        return;
      }
      if (!reply) {
        setNotificationFeedback('assistantResponseNotificationReplyEmpty');
        return;
      }

      const current = useChatStore.getState();
      if (current.activeConversationId !== activation.conversationId || current.isSending) {
        preserveReply();
        setNotificationFeedback('assistantResponseNotificationReplyNotSent');
        return;
      }
      try {
        const [messages, questionResult] = await Promise.all([
          getMessages(activation.conversationId),
          localServiceClient.call('chat.questions.list', { conversationId: activation.conversationId }),
        ]);
        const lastMessage = messages.at(-1);
        if (
          !lastMessage ||
          lastMessage.id !== activation.assistantMessageId ||
          lastMessage.role !== 'assistant' ||
          lastMessage.status !== 'completed'
        ) {
          preserveReply();
          setNotificationFeedback('assistantResponseNotificationReplyUnavailable');
          return;
        }
        if (!Array.isArray(questionResult.questions)) throw new Error('The pending question list is invalid.');
        const questionGroups = questionResult.questions.map(parseAgentQuestionGroup);
        if (questionGroups.length > 0) {
          preserveReply();
          setNotificationFeedback('assistantResponseNotificationReplyNotSent');
          return;
        }
        const sent = await useChatStore.getState().sendMessage(reply);
        if (!sent) {
          preserveReply();
          setNotificationFeedback('assistantResponseNotificationReplyNotSent');
        }
      } catch {
        preserveReply();
        setNotificationFeedback('assistantResponseNotificationReplyUnavailable');
      }
    },
    [navigateSurface, requestTemporaryChatExit],
  );

  const notificationActivationHandlerRef = useRef(handleNotificationActivation);
  notificationActivationHandlerRef.current = handleNotificationActivation;

  const clearQuestionFocusTarget = useCallback(() => setQuestionFocusTarget(null), []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<unknown>('openchat-notification-activated', (event) => {
      if (disposed) return;
      const activation = parseNotificationActivation(event.payload);
      if (activation) {
        void notificationActivationHandlerRef.current(activation).catch(() => {
          setNotificationFeedback(
            activation.kind === 'openQuestion'
              ? 'userQuestionUnavailable'
              : 'assistantResponseNotificationReplyUnavailable',
          );
        });
      }
    })
      .then((stopListening) => {
        if (disposed) stopListening();
        else {
          unlisten = stopListening;
          void invoke<unknown>('take_pending_notification_activations')
            .then((pending) => {
              if (disposed) return;
              if (!Array.isArray(pending)) throw new Error('Pending notification activations are invalid.');
              for (const value of pending) {
                const activation = parseNotificationActivation(value);
                if (activation) {
                  void notificationActivationHandlerRef.current(activation).catch(() => {
                    setNotificationFeedback(
                      activation.kind === 'openQuestion'
                        ? 'userQuestionUnavailable'
                        : 'assistantResponseNotificationReplyUnavailable',
                    );
                  });
                }
              }
            })
            .catch(() => {
              if (!disposed) setNotificationFeedback('notificationDeliveryFailed');
            });
        }
      })
      .catch(() => {
        if (!disposed) setNotificationFeedback('notificationDeliveryFailed');
      });
    const onNotificationError = () => {
      if (!disposed) setNotificationFeedback('notificationDeliveryFailed');
    };
    window.addEventListener('openchat-notification-error', onNotificationError);
    const onFeedback = (event: Event) => {
      if (disposed) return;
      if (!(event instanceof CustomEvent) || !isNotificationFeedbackKey(event.detail)) return;
      setNotificationFeedback(event.detail);
    };
    window.addEventListener('openchat-notification-feedback', onFeedback);
    return () => {
      disposed = true;
      unlisten?.();
      void invoke('set_notification_frontend_unready').catch(() => {
        window.dispatchEvent(new CustomEvent('openchat-notification-error'));
      });
      window.removeEventListener('openchat-notification-error', onNotificationError);
      window.removeEventListener('openchat-notification-feedback', onFeedback);
    };
  }, []);

  useEffect(() => {
    if (!notificationFeedback) return;
    const timer = window.setTimeout(() => setNotificationFeedback(null), 8000);
    return () => window.clearTimeout(timer);
  }, [notificationFeedback]);

  const heading = {
    chats: t('chats'),
    models: t('modelLibrary'),
    settings: t('settings'),
  }[activeSurface];

  const changeThemePreference = (nextTheme: ThemePreference) => {
    const nextIsDark = nextTheme === 'dark' || (nextTheme === 'system' && systemIsDark);
    updateThemeWithTransition(
      () => setTheme(nextTheme),
      nextIsDark !== isDark && lastInteractionMethod.current === 'pointer',
    );
  };

  const toggleTheme = () => {
    const nextTheme: ThemePreference = isDark ? 'light' : 'dark';
    try {
      localStorage.setItem('openchat.theme', nextTheme);
    } catch {
      // The selected theme still applies for this session when storage is unavailable.
    }
    changeThemePreference(nextTheme);
  };

  return (
    <div
      className="app-shell h-screen min-h-desktop-window overflow-hidden bg-background text-text"
      onKeyDownCapture={() => {
        lastInteractionMethod.current = 'keyboard';
        document.documentElement.dataset.motionInput = 'keyboard';
      }}
      onPointerDownCapture={() => {
        lastInteractionMethod.current = 'pointer';
        document.documentElement.dataset.motionInput = 'pointer';
      }}
    >
      <TitleBar
        canGoBack={surfaceHistory.index > 0}
        canGoForward={surfaceHistory.index < surfaceHistory.surfaces.length - 1}
        onGoBack={() => moveInHistory(-1)}
        onGoForward={() => moveInHistory(1)}
      />
      <NavigationRail activeSurface={activeSurface} isDark={isDark} onNavigate={navigate} onToggleTheme={toggleTheme} />
      <div className="app-shell-main relative flex min-h-0 min-w-0">
        <main
          aria-label={heading}
          className="app-main-surface relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
          data-motion-input={lastInteractionMethod.current}
          key={activeSurface}
          ref={surfaceHeadingRef}
          tabIndex={-1}
        >
          {activeSurface === 'chats' ? (
            <Tabs
              className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
              onValueChange={selectChatTab}
              value={activeChatTab}
            >
              <ConversationActionsHeader
                activeConversationId={activeConversationId}
                conversations={conversations}
                onNewChat={startWorkspaceChat}
                onOpenSearch={openHistorySearch}
                onStartTemporaryChat={startWorkspaceTemporaryChat}
              />
              <TabsContent className="relative flex min-h-0 min-w-0 flex-1 overflow-hidden" value={activeChatTab}>
                <ConversationPane
                  isLargeText={appearance.conversationTextSize === 'large'}
                  notificationFeedbackKey={notificationFeedback}
                  onDismissNotificationFeedback={() => setNotificationFeedback(null)}
                  onOpenSettings={() => navigate('settings')}
                  onQuestionFocusHandled={clearQuestionFocusTarget}
                  onRequestTemporaryExit={requestTemporaryChatExit}
                  questionFocusTarget={questionFocusTarget}
                />
              </TabsContent>
            </Tabs>
          ) : activeSurface === 'models' ? (
            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-surface">
              <Suspense
                fallback={
                  <p aria-live="polite" className="px-6 py-8 text-sm text-secondary-text">
                    {t('modelsLoading')}
                  </p>
                }
              >
                <LibraryPage />
              </Suspense>
            </div>
          ) : (
            <div className="min-w-0 flex-1 overflow-hidden">
              <SettingsPage
                appearance={appearance}
                onOpenConversation={openStatisticsConversation}
                onAppearanceChange={(updated) => setAppearance((current) => ({ ...current, ...updated }))}
                onThemeChange={changeThemePreference}
                theme={theme}
              />
            </div>
          )}
        </main>
      </div>
      <footer className="app-shell-chrome app-shell-footer flex select-none items-center justify-between gap-2 px-2 py-1 text-xs font-medium text-secondary-text">
        <span className="flex items-center gap-2">
          <a
            aria-label="OpenChat GitHub repository"
            className="inline-flex items-center rounded-xs hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
            href="https://github.com/alztrk/openchat"
            rel="noreferrer"
            target="_blank"
            title="GitHub"
          >
            <Github aria-hidden="true" size={14} />
            <span className="ml-1">GitHub</span>
          </a>
          <span>{appVersion ? `v${appVersion}` : null}</span>
        </span>
        <span className="flex select-none items-center gap-1.5">
          <BrandMark size="footer" />
          <span>OpenChat</span>
        </span>
      </footer>
      <ConversationSearchDialog
        initialQuery={historySearchInitialQuery}
        onOpenChange={(open) => {
          setHistorySearchOpen(open);
          if (!open) setHistorySearchInitialQuery('');
        }}
        onSelectResult={selectHistorySearchResult}
        open={historySearchOpen}
        projects={projects}
        projectsStatus={projectsStatus}
      />
      <TemporaryChatExitDialog
        canSave={temporaryChatMessageCount > 0 && temporaryChat?.route !== null}
        errorMessage={
          temporaryChatErrorCode === 'temporaryChatSaveFailed' ||
          temporaryChatErrorCode === 'temporaryChatSaveCleanupFailed'
            ? t(temporaryChatErrorCode)
            : null
        }
        isBusy={isSavingTemporaryChat}
        onDiscard={discardTemporaryChatAndContinue}
        onOpenChange={closeTemporaryChatExitDialog}
        onSave={() => void saveTemporaryChatAndContinue()}
        open={temporaryExitIntent !== null}
      />
    </div>
  );
}

function parseNotificationActivation(value: unknown): NotificationActivation | null {
  if (!isObject(value) || !isNotificationIdentifier(value.conversationId)) return null;
  if (value.kind === 'openConversation') {
    return { kind: value.kind, conversationId: value.conversationId };
  }
  if (value.kind === 'openQuestion' && isNotificationIdentifier(value.groupId)) {
    return {
      kind: value.kind,
      conversationId: value.conversationId,
      groupId: value.groupId,
    };
  }
  if (
    value.kind === 'replyToAssistant' &&
    isNotificationIdentifier(value.assistantMessageId) &&
    (value.replyText === undefined || value.replyText === null || typeof value.replyText === 'string') &&
    (value.replyInputTooLong === undefined || typeof value.replyInputTooLong === 'boolean')
  ) {
    return {
      kind: value.kind,
      conversationId: value.conversationId,
      assistantMessageId: value.assistantMessageId,
      replyText: typeof value.replyText === 'string' ? value.replyText : null,
      replyInputTooLong: value.replyInputTooLong === true,
    };
  }
  return null;
}

function isNotificationFeedbackKey(value: unknown): value is NotificationFeedbackKey {
  return (
    value === 'assistantResponseNotificationReplyEmpty' ||
    value === 'assistantResponseNotificationReplyTooLong' ||
    value === 'assistantResponseNotificationReplyUnavailable' ||
    value === 'assistantResponseNotificationReplyNotSent' ||
    value === 'userQuestionUnavailable' ||
    value === 'notificationDeliveryFailed' ||
    value === 'notificationTestFailed'
  );
}

function isNotificationIdentifier(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(value);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
