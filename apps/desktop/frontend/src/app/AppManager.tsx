import {
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { flushSync } from 'react-dom';
import type { ThemePreference } from '@features/app-settings/components/GeneralSettingsSection';
import type { SettingsSection } from '@features/app-settings/components/SettingsPage';
import {
  type AppearancePreferences,
  getAppearanceCssValues,
  readAppearancePreferences,
} from '@features/app-settings/data/appearancePreferences';
import {
  readCloseWindowBehavior,
  readInitialSurface,
  writeCloseWindowBehavior,
  writeLastSurface,
} from '@features/app-settings/data/appPreferences';
import { useAssistantExecutionPreferencesStore } from '@shared/lib/preferences/assistantExecutionPreferences';
import { useChatDisplayPreferencesStore } from '@shared/lib/preferences/chatDisplayPreferences';
import { useKeyboardShortcutsStore } from '@shared/lib/preferences/keyboardShortcutsStore';
import { useDesignPreferencesStore } from '@app/runtime/designPreferences';
import {
  OAUTH_PLAN_CHECK_INTERVAL_MS,
  refreshOAuthPlans,
  shouldRefreshOAuthPlansOnFocus,
} from '@app/runtime/oauthPlanMonitor';
import { initializeActiveSecurityScans } from '@features/security-scans/data/securityScanRuntime';
import {
  resetNotificationWindowFocus,
  setNotificationWindowFocus,
} from '@app/runtime/notificationRuntime';
import { localServiceClient } from '@shared/lib/platform/local-service';
import { unifiedStorage } from '@shared/lib/platform/storage/unifiedStorage';
import { getCurrentWindow, getVersion, invoke, isTauri, listen } from '@shared/lib/platform/tauri';
import { useChatStore } from '@app/runtime/chatStore';
import { getMessages } from '@app/runtime/chatStorage';
import { parseAgentQuestionGroup } from '@shared/lib/preferences/agentQuestions';
import { clampWorkspacePanelRatio, readWorkspacePanelRatio, writeWorkspacePanelRatio } from '@shared/lib/preferences/sidebarPreferences';

import type { AppSurface } from '@widgets/navigation-rail';

// ---------------------------------------------------------------------------
// Internal helpers (not exported)
// ---------------------------------------------------------------------------

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNotificationIdentifier(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(value);
}

function readThemePreference(): ThemePreference {
  try {
    const preference = unifiedStorage.getItem('openchat.theme');
    if (preference === 'system' || preference === 'light' || preference === 'dark') {
      return preference;
    }
  } catch {
    return 'system';
  }
  return 'system';
}

export type NotificationFeedbackKey =
  | 'assistantResponseNotificationReplyEmpty'
  | 'assistantResponseNotificationReplyTooLong'
  | 'assistantResponseNotificationReplyUnavailable'
  | 'assistantResponseNotificationReplyNotSent'
  | 'userQuestionUnavailable'
  | 'notificationDeliveryFailed'
  | 'notificationTestFailed';

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

interface NotificationActivation {
  kind: 'openConversation' | 'openQuestion' | 'replyToAssistant';
  conversationId: string;
  groupId?: string;
  assistantMessageId?: string;
  replyText?: string | null;
  replyInputTooLong?: boolean;
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

interface SurfaceHistory {
  surfaces: AppSurface[];
  index: number;
}

// ---------------------------------------------------------------------------
// updateThemeWithTransition
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// useAppManager hook
// ---------------------------------------------------------------------------

export function useAppManager() {
  // ---- Theme & Appearance ------------------------------------------------
  const [theme, setTheme] = useState<ThemePreference>(readThemePreference);
  const [themeSaving, setThemeSaving] = useState(false);
  const [themeSaveFailed, setThemeSaveFailed] = useState(false);
  const [systemIsDark, setSystemIsDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  const [appearance, setAppearance] = useState<AppearancePreferences>(readAppearancePreferences);
  const isDark = theme === 'dark' || (theme === 'system' && systemIsDark);

  // ---- Surface / Navigation History --------------------------------------
  const [surfaceHistory, setSurfaceHistory] = useState<SurfaceHistory>({
    surfaces: [readInitialSurface()],
    index: 0,
  });
  const activeSurface = surfaceHistory.surfaces[surfaceHistory.index] ?? 'chats';

  // ---- UI State ----------------------------------------------------------
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [isRightPanelOpen, setIsRightPanelOpen] = useState(false);
  const [workspaceRatio, setWorkspaceRatio] = useState<number>(readWorkspacePanelRatio);

  // ---- Request Queues ----------------------------------------------------
  const [libraryTabRequest, setLibraryTabRequest] = useState<{
    tab: 'models' | 'agents';
    requestId: number;
  }>({ tab: 'models', requestId: 0 });
  const [settingsSectionRequest, setSettingsSectionRequest] = useState<{
    section: SettingsSection;
    requestId: number;
  }>({ section: 'connections', requestId: 0 });

  // ---- Temporary Chat ----------------------------------------------------
  const [temporaryExitIntent, setTemporaryExitIntent] = useState<(() => void | Promise<void>) | null>(null);
  const [isSavingTemporaryChat, setIsSavingTemporaryChat] = useState(false);

  // ---- Notification State ------------------------------------------------
  const [notificationFeedback, setNotificationFeedback] = useState<NotificationFeedbackKey | null>(null);
  const [questionFocusTarget, setQuestionFocusTarget] = useState<{
    conversationId: string;
    groupId: string;
  } | null>(null);

  // ---- Refs --------------------------------------------------------------
  const lastInteractionMethod = useRef<'keyboard' | 'pointer'>('pointer');
  const goalBudgetPauseAttemptKey = useRef<string | null>(null);
  const notificationActivationHandlerRef = useRef<
    (activation: NotificationActivation) => Promise<void>
  >(async () => {
    /* noop */
  });

  // ---- Store accessors ---------------------------------------------------
  const initializeChat = useChatStore((state) => state.initialize);
  const initializeKeyboardShortcuts = useKeyboardShortcutsStore((state) => state.initialize);
  const initializeAssistantExecutionPreferences = useAssistantExecutionPreferencesStore(
    (state) => state.initialize,
  );
  const maxGoalRuntimeMinutes = useAssistantExecutionPreferencesStore(
    (state) => state.preferences.maxGoalRuntimeMinutes,
  );
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const activeGoal = useChatStore((state) => state.activeGoal);
  const isChangingGoal = useChatStore((state) => state.isChangingGoal);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);

  // ---------------------------------------------------------------------------
  // 1. Theme management – changeThemePreference
  // ---------------------------------------------------------------------------
  const changeThemePreference = useCallback((nextTheme: ThemePreference) => {
    setThemeSaveFailed(false);
    const nextIsDark = nextTheme === 'dark' || (nextTheme === 'system' && systemIsDark);
    updateThemeWithTransition(
      () => setTheme(nextTheme),
      nextIsDark !== isDark && lastInteractionMethod.current === 'pointer',
    );
  }, [isDark, systemIsDark]);

  // ---------------------------------------------------------------------------
  // 2. Theme management – toggleTheme
  // ---------------------------------------------------------------------------
  const toggleTheme = useCallback(async () => {
    if (themeSaving) return;
    const nextTheme: ThemePreference = isDark ? 'light' : 'dark';
    setThemeSaving(true);
    setThemeSaveFailed(false);
    try {
      await unifiedStorage.setPersistedItem('openchat.theme', nextTheme);
      changeThemePreference(nextTheme);
    } catch {
      setThemeSaveFailed(true);
    } finally {
      setThemeSaving(false);
    }
  }, [changeThemePreference, isDark, themeSaving]);

  // ---------------------------------------------------------------------------
  // 3. Appearance management – CSS values
  // ---------------------------------------------------------------------------
  useLayoutEffect(() => {
    const root = document.documentElement;
    const values = getAppearanceCssValues(appearance);
    root.style.setProperty('--conversation-reading-width', values.conversationWidth);
    root.style.setProperty('--app-text-scale', values.textScale);
    root.style.fontSize = values.appScale;
    root.style.setProperty('--spacing', values.appSpacing);
    root.style.setProperty('--app-font-family', values.fontFamily);
  }, [appearance]);

  // ---------------------------------------------------------------------------
  // 4. Close window behavior
  // ---------------------------------------------------------------------------
  useEffect(() => {
    void writeCloseWindowBehavior(readCloseWindowBehavior()).catch((cause: unknown) => {
      globalThis.reportError(new Error('The window close behavior could not be applied.', { cause }));
    });
  }, []);

  // ---------------------------------------------------------------------------
  // 5. Temporary chat management
  // ---------------------------------------------------------------------------
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

  const discardTemporaryChatAndContinue = useCallback(() => {
    if (!temporaryExitIntent || isSavingTemporaryChat) return;
    const action = temporaryExitIntent;
    useChatStore.getState().discardTemporaryChat();
    setTemporaryExitIntent(null);
    void Promise.resolve(action()).catch(() => {
      useChatStore.setState({ errorCode: 'messageHistoryLoadFailed' });
    });
  }, [temporaryExitIntent, isSavingTemporaryChat]);

  const saveTemporaryChatAndContinue = useCallback(async () => {
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
  }, [temporaryExitIntent, isSavingTemporaryChat]);

  const closeTemporaryChatExitDialog = useCallback((open: boolean) => {
    if (!open && !isSavingTemporaryChat) setTemporaryExitIntent(null);
  }, [isSavingTemporaryChat]);

  // ---------------------------------------------------------------------------
  // 6. Navigation / Surface history
  // ---------------------------------------------------------------------------
  const navigateSurface = useCallback((surface: AppSurface) => {
    try {
      writeLastSurface(surface);
    } catch (cause: unknown) {
      globalThis.reportError(new Error('The last active workspace could not be saved.', { cause }));
    }
    setSurfaceHistory((current) => {
      if (current.surfaces[current.index] === surface) return current;
      const surfaces = [...current.surfaces.slice(0, current.index + 1), surface];
      if (surfaces.length > 50) surfaces.shift();
      return { surfaces, index: surfaces.length - 1 };
    });
  }, []);

  const moveInHistory = useCallback(
    (offset: number) => {
      requestTemporaryChatExit(() => {
        setSurfaceHistory((current) => {
          const nextIndex = current.index + offset;
          return current.surfaces[nextIndex] ? { ...current, index: nextIndex } : current;
        });
      });
    },
    [requestTemporaryChatExit],
  );

  const navigate = useCallback(
    (surface: AppSurface, surfaceHeadingRef: HTMLElement | null) => {
      if (surface === activeSurface) {
        surfaceHeadingRef?.focus();
        return;
      }
      requestTemporaryChatExit(() => {
        if (surface === 'models') {
          setLibraryTabRequest((current) => ({ tab: 'models', requestId: current.requestId + 1 }));
        }
        navigateSurface(surface);
      });
    },
    [activeSurface, navigateSurface, requestTemporaryChatExit],
  );

  // ---------------------------------------------------------------------------
  // 7. Notification feedback – auto clear
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!notificationFeedback) return;
    const timer = window.setTimeout(() => setNotificationFeedback(null), 8000);
    return () => window.clearTimeout(timer);
  }, [notificationFeedback]);

  // ---------------------------------------------------------------------------
  // 8. Notification window focus
  // ---------------------------------------------------------------------------
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

  // ---------------------------------------------------------------------------
  // 9. Tauri event listeners (app-setting-changed, conversation-changed)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlistenSetting: (() => void) | undefined;
    let unlistenConversation: (() => void) | undefined;

    void listen<{ key: string; value_json: string }>('openchat://app-setting-changed', (event) => {
      if (disposed) return;
      if (event.payload.key.startsWith('appearance.')) {
        setAppearance(readAppearancePreferences());
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenSetting = unlisten;
    });

    void listen<{ conversation_id: string; action: string }>('openchat://conversation-changed', () => {
      if (disposed) return;
      void useChatStore.getState().initialize();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenConversation = unlisten;
    });

    return () => {
      disposed = true;
      unlistenSetting?.();
      unlistenConversation?.();
    };
  }, []);

  // ---------------------------------------------------------------------------
  // 10. System theme listener (prefers-color-scheme)
  // ---------------------------------------------------------------------------
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

  // ---------------------------------------------------------------------------
  // 11. OAuth plan refresh (useEffect with interval)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!isTauri() || serviceStatus !== 'ready') return;
    let disposed = false;

    const runCheck = () => {
      if (disposed || document.visibilityState === 'hidden') return;
      void refreshOAuthPlans().then((summary) => {
        if (!disposed && summary.planOrAuthChanged) {
          void useChatStore.getState().refreshModels(true);
        }
      });
    };

    const initialTimer = window.setTimeout(runCheck, 2500);
    const intervalTimer = window.setInterval(runCheck, OAUTH_PLAN_CHECK_INTERVAL_MS);

    const handleVisibilityOrFocus = () => {
      if (document.visibilityState === 'visible' && shouldRefreshOAuthPlansOnFocus()) {
        runCheck();
      }
    };

    window.addEventListener('focus', handleVisibilityOrFocus);
    document.addEventListener('visibilitychange', handleVisibilityOrFocus);

    return () => {
      disposed = true;
      window.clearTimeout(initialTimer);
      window.clearInterval(intervalTimer);
      window.removeEventListener('focus', handleVisibilityOrFocus);
      document.removeEventListener('visibilitychange', handleVisibilityOrFocus);
    };
  }, [serviceStatus]);

  // ---------------------------------------------------------------------------
  // 12. Security scans init (useEffect with retry)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!isTauri() || serviceStatus !== 'ready') return;
    let disposed = false;
    let retryTimer: number | undefined;
    const retryDelaysMs = [1000, 5000] as const;

    const initializeScans = async (attempt: number): Promise<void> => {
      try {
        await initializeActiveSecurityScans();
      } catch (error) {
        if (disposed) return;
        const retryDelay = retryDelaysMs[attempt];
        if (retryDelay !== undefined) {
          retryTimer = window.setTimeout(() => void initializeScans(attempt + 1), retryDelay);
          return;
        }
        globalThis.reportError(new Error('Persisted security scans could not be restored.', { cause: error }));
      }
    };

    void initializeScans(0);
    return () => {
      disposed = true;
      if (retryTimer !== undefined) window.clearTimeout(retryTimer);
    };
  }, [serviceStatus]);

  // ---------------------------------------------------------------------------
  // 13. Keyboard shortcuts init (store initializations)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    initializeKeyboardShortcuts();
    useChatDisplayPreferencesStore.getState().initialize();
    initializeAssistantExecutionPreferences();
    useDesignPreferencesStore.getState().initialize();
  }, [initializeAssistantExecutionPreferences, initializeKeyboardShortcuts]);

  // ---------------------------------------------------------------------------
  // 14. Goal budget timer
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (activeGoal?.status !== 'running' || maxGoalRuntimeMinutes === 0) {
      goalBudgetPauseAttemptKey.current = null;
      return;
    }
    if (isChangingGoal) return;
    const attemptKey = `${activeGoal.runId}:${maxGoalRuntimeMinutes}`;
    if (goalBudgetPauseAttemptKey.current === attemptKey) return;
    const budgetMs = maxGoalRuntimeMinutes * 60_000;
    const remainingMs = budgetMs - (Date.now() - activeGoal.startedAtUnixMs);
    const timer = window.setTimeout(
      () => {
        const current = useChatStore.getState();
        if (
          current.activeGoal?.runId === activeGoal.runId &&
          current.activeGoal.status === 'running' &&
          Date.now() - current.activeGoal.startedAtUnixMs >= budgetMs
        ) {
          goalBudgetPauseAttemptKey.current = attemptKey;
          void current.pauseActiveGoal('budget_exceeded');
        }
      },
      Math.max(0, remainingMs),
    );
    return () => window.clearTimeout(timer);
  }, [activeGoal, isChangingGoal, maxGoalRuntimeMinutes]);

  // ---------------------------------------------------------------------------
  // 15. Window focus/blur (chat-store init)
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (isTauri()) void initializeChat();
  }, [initializeChat]);

  // ---------------------------------------------------------------------------
  // 16. App version
  // ---------------------------------------------------------------------------
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

  // ---------------------------------------------------------------------------
  // 17. Dark class sync (useLayoutEffect)
  // ---------------------------------------------------------------------------
  useLayoutEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
  }, [isDark]);

  // ---------------------------------------------------------------------------
  // 18. Notification activation handler (ref + listeners)
  // ---------------------------------------------------------------------------
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

  notificationActivationHandlerRef.current = handleNotificationActivation;

  // Notification activation listeners
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

  // ---------------------------------------------------------------------------
  // 19. Keyboard shortcuts & interaction tracking refs
  // ---------------------------------------------------------------------------
  const handleKeyboardInteraction = useCallback(() => {
    lastInteractionMethod.current = 'keyboard';
    document.documentElement.dataset.motionInput = 'keyboard';
  }, []);

  const handlePointerInteraction = useCallback(() => {
    lastInteractionMethod.current = 'pointer';
    document.documentElement.dataset.motionInput = 'pointer';
  }, []);

  // ---------------------------------------------------------------------------
  // 20. Workspace resize handlers
  // ---------------------------------------------------------------------------
  const handleWorkspaceResizePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>, containerRef: React.RefObject<HTMLDivElement | null>) => {
      if (event.button !== 0) return;
      event.preventDefault();

      const handlePointerMove = (moveEvent: PointerEvent) => {
        const container = containerRef.current;
        if (!container) return;
        const bounds = container.getBoundingClientRect();
        if (bounds.width <= 0) return;
        const nextRatio = clampWorkspacePanelRatio(((moveEvent.clientX - bounds.left) / bounds.width) * 100);
        setWorkspaceRatio(nextRatio);
      };

      const handlePointerUp = (upEvent: PointerEvent) => {
        const container = containerRef.current;
        if (!container) return;
        const bounds = container.getBoundingClientRect();
        if (bounds.width > 0) {
          const finalRatio = clampWorkspacePanelRatio(((upEvent.clientX - bounds.left) / bounds.width) * 100);
          setWorkspaceRatio(finalRatio);
          writeWorkspacePanelRatio(finalRatio);
        }
        window.removeEventListener('pointermove', handlePointerMove);
        window.removeEventListener('pointerup', handlePointerUp);
      };

      window.addEventListener('pointermove', handlePointerMove);
      window.addEventListener('pointerup', handlePointerUp);
    },
    [],
  );

  const handleWorkspaceResizeKeyDown = useCallback((event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowLeft' ? -5 : 5;
    setWorkspaceRatio((current) => {
      const next = clampWorkspacePanelRatio(current + delta);
      writeWorkspacePanelRatio(next);
      return next;
    });
  }, []);

  // ---------------------------------------------------------------------------
  // 21. Sidebar toggle
  // ---------------------------------------------------------------------------
  const handleToggleSidebar = useCallback(
    () => setIsSidebarOpen((open) => !open),
    [],
  );

  // ---------------------------------------------------------------------------
  // 22. Chat action helpers (used by UI components)
  // ---------------------------------------------------------------------------
  const startWorkspaceChat = useCallback(() => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      startNewChat();
    });
  }, [navigateSurface, requestTemporaryChatExit, startNewChat]);

  const openAgentLibrary = useCallback(() => {
    requestTemporaryChatExit(() => {
      setLibraryTabRequest((current) => ({ tab: 'agents', requestId: current.requestId + 1 }));
      navigateSurface('models');
    });
  }, [navigateSurface, requestTemporaryChatExit]);

  // ---------------------------------------------------------------------------
  // 23. Question focus target clear
  // ---------------------------------------------------------------------------
  const clearQuestionFocusTarget = useCallback(() => setQuestionFocusTarget(null), []);

  // ---------------------------------------------------------------------------
  // Return value – all state, derived values, and actions
  // ---------------------------------------------------------------------------
  return {
    // Theme
    theme,
    systemIsDark,
    isDark,
    themeSaving,
    themeSaveFailed,
    changeThemePreference,
    toggleTheme,

    // Appearance
    appearance,
    setAppearance,

    // Surface / Navigation
    surfaceHistory,
    activeSurface,
    navigateSurface,
    moveInHistory,
    navigate,

    // UI State
    isCommandPaletteOpen,
    setIsCommandPaletteOpen,
    appVersion,
    isSidebarOpen,
    setIsSidebarOpen,
    isRightPanelOpen,
    setIsRightPanelOpen,
    workspaceRatio,
    libraryTabRequest,
    setLibraryTabRequest,
    settingsSectionRequest,
    setSettingsSectionRequest,

    // Temporary Chat
    temporaryExitIntent,
    isSavingTemporaryChat,
    requestTemporaryChatExit,
    discardTemporaryChatAndContinue,
    saveTemporaryChatAndContinue,
    closeTemporaryChatExitDialog,

    // Notification
    notificationFeedback,
    setNotificationFeedback,
    questionFocusTarget,
    clearQuestionFocusTarget,

    // Interaction tracking
    lastInteractionMethodRef: lastInteractionMethod,
    handleKeyboardInteraction,
    handlePointerInteraction,

    // Workspace resize
    handleWorkspaceResizePointerDown,
    handleWorkspaceResizeKeyDown,

    // Sidebar
    handleToggleSidebar,

    // Chat actions
    startWorkspaceChat,
    openAgentLibrary,
    activeConversationId,
    serviceStatus,
    startNewChat,
    startTemporaryChat,
  };
}

// ---------------------------------------------------------------------------
// AppManager component – minimal wrapper that just uses the hook
// ---------------------------------------------------------------------------

export function AppManager() {
  useAppManager();
  return null;
}
