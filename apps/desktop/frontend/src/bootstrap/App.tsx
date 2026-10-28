import { Github, MessageSquare, PanelRight } from 'lucide-react';
import {
  lazy,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  Suspense,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { flushSync } from 'react-dom';
import { useTranslation } from 'react-i18next';
import type {
  RightWorkspaceTab,
  WorkspaceFileRequest,
  WorkspaceFileSelection,
  WorkspacePanelRequest,
} from '@/modules/conversations/components/ChatFileChangesPanel';
import { parseAgentQuestionGroup } from '@/modules/conversations/data/agentQuestions';
import { getMessages } from '@/modules/conversations/data/chatStorage';
import { setPendingComposerDraft } from '@/modules/conversations/data/pendingComposerDraft';
import {
  clampWorkspacePanelRatio,
  readWorkspacePanelRatio,
  writeWorkspacePanelRatio,
} from '@/modules/conversations/data/sidebarPreferences';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { useDesignPreferencesStore } from '@/modules/design/data/designPreferences';
import {
  resetNotificationWindowFocus,
  setNotificationWindowFocus,
} from '@/modules/notifications/data/notificationRuntime';
import {
  OAUTH_PLAN_CHECK_INTERVAL_MS,
  refreshOAuthPlans,
  shouldRefreshOAuthPlansOnFocus,
} from '@/modules/providers/data/oauthPlanMonitor';
import { initializeActiveSecurityScans } from '@/modules/security-scans/data/securityScanRuntime';
import type { ThemePreference } from '@/modules/settings/components/GeneralSettingsSection';
import type { SettingsSection } from '@/modules/settings/components/SettingsPage';
import {
  type AppearancePreferences,
  getAppearanceCssValues,
  readAppearancePreferences,
} from '@/modules/settings/data/appearancePreferences';
import {
  readCloseWindowBehavior,
  readInitialSurface,
  writeCloseWindowBehavior,
  writeLastSurface,
} from '@/modules/settings/data/appPreferences';
import { useAssistantExecutionPreferencesStore } from '@/modules/settings/data/assistantExecutionPreferences';
import type { AssistantAgentSnapshot } from '@/modules/settings/data/assistantProfiles';
import { useChatDisplayPreferencesStore } from '@/modules/settings/data/chatDisplayPreferences';
import { useKeyboardShortcutsStore } from '@/modules/settings/stores/keyboardShortcutsStore';
import { localServiceClient } from '@/platform/local-service';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { getCurrentWindow, getVersion, invoke, isTauri, listen } from '@/platform/tauri';
import { BrandMark } from '@/shell/BrandMark';
import { ConversationActionsHeader } from '@/shell/ConversationActionsHeader';
import { type AppSurface, NavigationRail } from '@/shell/NavigationRail';
import { TitleBar } from '@/shell/TitleBar';
import { AppModals } from './AppModals';
import { useAppDragDrop } from './useAppDragDrop';
import { useAppKeyboardShortcuts } from './useAppKeyboardShortcuts';

const LibraryPage = lazy(() =>
  import('@/modules/model-library/components/LibraryPage').then((module) => ({ default: module.LibraryPage })),
);
const ConversationPane = lazy(() =>
  import('@/modules/conversations/components/ConversationPane').then((module) => ({
    default: module.ConversationPane,
  })),
);
const ConversationSidebar = lazy(() =>
  import('@/modules/conversations/components/ConversationSidebar').then((module) => ({
    default: module.ConversationSidebar,
  })),
);
const ChatFileChangesPanel = lazy(() =>
  import('@/modules/conversations/components/ChatFileChangesPanel').then((module) => ({
    default: module.ChatFileChangesPanel,
  })),
);
const DesignPage = lazy(() =>
  import('@/modules/design/components/DesignPage').then((module) => ({ default: module.DesignPage })),
);
const SecurityScanPage = lazy(() =>
  import('@/modules/security-scans/components/SecurityScanPage').then((module) => ({
    default: module.SecurityScanPage,
  })),
);
const SettingsPage = lazy(() =>
  import('@/modules/settings/components/SettingsPage').then((module) => ({ default: module.SettingsPage })),
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
    const preference = unifiedStorage.getItem('openchat.theme');
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
    surfaces: [readInitialSurface()],
    index: 0,
  });
  const [libraryTabRequest, setLibraryTabRequest] = useState<{
    tab: 'models' | 'agents';
    requestId: number;
  }>({ tab: 'models', requestId: 0 });
  const [settingsSectionRequest, setSettingsSectionRequest] = useState<{
    section: SettingsSection;
    requestId: number;
  }>({ section: 'connections', requestId: 0 });
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const activeSurface = surfaceHistory.surfaces[surfaceHistory.index] ?? 'chats';
  const [theme, setTheme] = useState<ThemePreference>(readThemePreference);
  const [themeSaving, setThemeSaving] = useState(false);
  const [themeSaveFailed, setThemeSaveFailed] = useState(false);
  const [appearance, setAppearance] = useState<AppearancePreferences>(readAppearancePreferences);
  const [notificationFeedback, setNotificationFeedback] = useState<NotificationFeedbackKey | null>(null);
  const [questionFocusTarget, setQuestionFocusTarget] = useState<{ conversationId: string; groupId: string } | null>(
    null,
  );
  const [temporaryExitIntent, setTemporaryExitIntent] = useState<(() => void | Promise<void>) | null>(null);
  const [isSavingTemporaryChat, setIsSavingTemporaryChat] = useState(false);
  const [systemIsDark, setSystemIsDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [workspaceRatio, setWorkspaceRatio] = useState<number>(readWorkspacePanelRatio);
  const workspaceDropRef = useRef<HTMLDivElement>(null);
  const rightPanelShellRef = useRef<HTMLDivElement>(null);
  const surfaceHeadingRef = useRef<HTMLElement>(null);
  const lastInteractionMethod = useRef<'keyboard' | 'pointer'>('pointer');
  const goalBudgetPauseAttemptKey = useRef<string | null>(null);
  const initializeChat = useChatStore((state) => state.initialize);
  const initializeKeyboardShortcuts = useKeyboardShortcutsStore((state) => state.initialize);
  const initializeAssistantExecutionPreferences = useAssistantExecutionPreferencesStore((state) => state.initialize);
  const maxGoalRuntimeMinutes = useAssistantExecutionPreferencesStore(
    (state) => state.preferences.maxGoalRuntimeMinutes,
  );
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const activeGoal = useChatStore((state) => state.activeGoal);
  const isChangingGoal = useChatStore((state) => state.isChangingGoal);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);
  const isDark = theme === 'dark' || (theme === 'system' && systemIsDark);

  useEffect(() => {
    if (!isTauri()) return;
    const activeComputerUseCalls = new Set<string>();
    const activeRequests = new Set<string>();
    const computerUseRequests = new Set<string>();
    let hideTimer: number | undefined;
    const stopHideTimer = () => {
      if (hideTimer !== undefined) window.clearTimeout(hideTimer);
      hideTimer = undefined;
    };
    const setOverlay = (active: boolean) => {
      const accent = getComputedStyle(document.documentElement)
        .getPropertyValue('--accent')
        .trim()
        .split(/\s+/)
        .map(Number);
      const validAccent =
        accent.length === 3 && accent.every((channel) => Number.isInteger(channel) && channel >= 0 && channel <= 255)
          ? accent
          : [142, 86, 59];
      void invoke('set_computer_use_overlay', { active, accent: validAccent }).catch((error: unknown) => {
        globalThis.reportError(new Error('The Computer Use indicator could not be updated.', { cause: error }));
      });
    };
    const scheduleHide = () => {
      if (activeComputerUseCalls.size > 0 || computerUseRequests.size > 0) return;
      stopHideTimer();
      hideTimer = window.setTimeout(() => {
        if (activeComputerUseCalls.size === 0 && computerUseRequests.size === 0) setOverlay(false);
        hideTimer = undefined;
      }, 3000);
    };
    const onRequestStarted = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail)) return;
      const conversationId = event.detail.conversationId;
      if (typeof conversationId === 'string') activeRequests.add(conversationId);
    };
    const onRequestFinished = (event: Event) => {
      if (!(event instanceof CustomEvent) || !isObject(event.detail)) return;
      const conversationId = event.detail.conversationId;
      if (typeof conversationId !== 'string') return;
      activeRequests.delete(conversationId);
      if (computerUseRequests.delete(conversationId)) scheduleHide();
    };
    window.addEventListener('openchat-chat-request-started', onRequestStarted);
    window.addEventListener('openchat-chat-request-finished', onRequestFinished);
    const unsubscribe = localServiceClient.subscribe((event) => {
      if (event.name !== 'chat.tool.updated' || !isObject(event.data.toolActivity)) return;
      const activity = event.data.toolActivity;
      if (activity.name !== 'computer_use' || typeof activity.callId !== 'string') return;
      if (activity.status === 'running') {
        stopHideTimer();
        const conversationId = event.data.conversationId;
        if (typeof conversationId === 'string') {
          if (activeRequests.has(conversationId)) computerUseRequests.add(conversationId);
        }
        activeComputerUseCalls.add(activity.callId);
        setOverlay(true);
        return;
      }
      if (['completed', 'failed', 'denied', 'cancelled'].includes(String(activity.status))) {
        activeComputerUseCalls.delete(activity.callId);
        scheduleHide();
      }
    });
    return () => {
      unsubscribe();
      window.removeEventListener('openchat-chat-request-started', onRequestStarted);
      window.removeEventListener('openchat-chat-request-finished', onRequestFinished);
      stopHideTimer();
      setOverlay(false);
    };
  }, []);

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

  useEffect(() => {
    initializeKeyboardShortcuts();
    useChatDisplayPreferencesStore.getState().initialize();
    initializeAssistantExecutionPreferences();
    useDesignPreferencesStore.getState().initialize();
  }, [initializeAssistantExecutionPreferences, initializeKeyboardShortcuts]);

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

  useEffect(() => {
    void writeCloseWindowBehavior(readCloseWindowBehavior()).catch((cause: unknown) => {
      globalThis.reportError(new Error('The window close behavior could not be applied.', { cause }));
    });
  }, []);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const values = getAppearanceCssValues(appearance);
    root.style.setProperty('--conversation-reading-width', values.conversationWidth);
    root.style.setProperty('--app-text-scale', values.textScale);
    root.style.fontSize = values.appScale;
    root.style.setProperty('--spacing', values.appSpacing);
    root.style.setProperty('--app-font-family', values.fontFamily);
  }, [appearance]);

  useEffect(() => {
    if (isTauri()) void initializeChat();
  }, [initializeChat]);

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

  const openSearchConversation = useCallback(
    (conversationId: string, messageId?: string | null, searchQuery?: string | null) => {
      requestTemporaryChatExit(async () => {
        navigateSurface('chats');
        await useChatStore.getState().selectConversation(conversationId, messageId, searchQuery);
      });
    },
    [navigateSurface, requestTemporaryChatExit],
  );

  const navigate = useCallback(
    (surface: AppSurface) => {
      if (surface === activeSurface) {
        surfaceHeadingRef.current?.focus();
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

  const startWorkspaceChat = useCallback(() => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      startNewChat();
    });
  }, [navigateSurface, requestTemporaryChatExit, startNewChat]);

  const startWorkspaceAgentChat = (agent: AssistantAgentSnapshot) => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      useChatStore.getState().startNewChatWithAgent(agent);
    });
  };

  const openAgentLibrary = () => {
    requestTemporaryChatExit(() => {
      setLibraryTabRequest((current) => ({ tab: 'agents', requestId: current.requestId + 1 }));
      navigateSurface('models');
    });
  };

  const startWorkspaceTemporaryChat = () => {
    const current = useChatStore.getState();
    if (current.temporaryChat) {
      requestTemporaryChatExit(() => {
        navigateSurface('chats');
        current.startNewChat();
      });
    } else {
      requestTemporaryChatExit(() => {
        navigateSurface('chats');
        startTemporaryChat();
      });
    }
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
    design: t('design'),
    models: t('modelLibrary'),
    security: t('securityScansTitle'),
    settings: t('settings'),
  }[activeSurface];

  const changeThemePreference = (nextTheme: ThemePreference) => {
    setThemeSaveFailed(false);
    const nextIsDark = nextTheme === 'dark' || (nextTheme === 'system' && systemIsDark);
    updateThemeWithTransition(
      () => setTheme(nextTheme),
      nextIsDark !== isDark && lastInteractionMethod.current === 'pointer',
    );
  };

  const toggleTheme = async () => {
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
  };

  const [isRightPanelOpen, setIsRightPanelOpen] = useState(false);
  const [workspacePanelRequest, setWorkspacePanelRequest] = useState<WorkspacePanelRequest | null>(null);
  const [workspaceFileRequest, setWorkspaceFileRequest] = useState<{
    conversationId: string | null;
    request: WorkspaceFileRequest;
  } | null>(null);
  const workspacePanelRequestId = useRef(0);
  const workspaceFileRequestId = useRef(0);

  useEffect(() => {
    if (!isRightPanelOpen) return;
    const closePanelOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) setIsRightPanelOpen(false);
    };
    window.addEventListener('keydown', closePanelOnEscape);
    return () => window.removeEventListener('keydown', closePanelOnEscape);
  }, [isRightPanelOpen]);

  const handleTogglePrimaryRightPanel = useCallback(() => {
    if (isRightPanelOpen) {
      setIsRightPanelOpen(false);
      return;
    }
    setIsRightPanelOpen(true);
  }, [isRightPanelOpen]);

  useAppKeyboardShortcuts({
    activeSurface,
    handleTogglePrimaryRightPanel,
    navigateSurface,
    requestTemporaryChatExit,
    setIsCommandPaletteOpen,
    setIsSidebarOpen,
    startWorkspaceChat,
  });

  const handleOpenRightPanelForConversation = useCallback(
    (conversationId: string | null, tab: RightWorkspaceTab = 'changes') => {
      if (!conversationId) return;
      setWorkspaceFileRequest(null);
      setIsRightPanelOpen(true);
      setWorkspacePanelRequest({
        requestId: ++workspacePanelRequestId.current,
        kind: 'builtin',
        tab,
        conversationId,
      });
    },
    [],
  );

  const handleOpenConversationInPanel = useCallback(
    (conversationId: string) => {
      if (!conversations.some((conversation) => conversation.id === conversationId)) return;
      setWorkspaceFileRequest(null);
      setWorkspacePanelRequest({
        requestId: ++workspacePanelRequestId.current,
        kind: 'conversation',
        conversationId,
      });
      setIsRightPanelOpen(true);
    },
    [conversations],
  );

  const handleOpenWorkspaceFileForConversation = useCallback(
    (conversationId: string | null, file: WorkspaceFileSelection) => {
      setIsRightPanelOpen(true);
      setWorkspacePanelRequest({
        requestId: ++workspacePanelRequestId.current,
        kind: 'builtin',
        tab: 'changes',
        conversationId,
      });
      setWorkspaceFileRequest({
        conversationId,
        request: { ...file, conversationId, requestId: ++workspaceFileRequestId.current },
      });
    },
    [],
  );

  useEffect(() => {
    const onOpenPanelConversation = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail = isObject(event.detail) ? event.detail : null;
      const targetId = detail && typeof detail.conversationId === 'string' ? detail.conversationId : null;
      if (targetId) handleOpenConversationInPanel(targetId);
    };
    window.addEventListener('openchat-open-panel-conversation', onOpenPanelConversation);
    return () => window.removeEventListener('openchat-open-panel-conversation', onOpenPanelConversation);
  }, [handleOpenConversationInPanel]);

  useEffect(
    () =>
      localServiceClient.subscribe((event) => {
        if (
          event.name === 'chat.subagent.updated' &&
          event.data.status === 'running' &&
          event.data.phase === 'queued' &&
          typeof event.data.conversationId === 'string'
        ) {
          handleOpenRightPanelForConversation(event.data.conversationId, 'subagents');
        }
      }),
    [handleOpenRightPanelForConversation],
  );

  const handleOpenConversationInPrimary = useCallback(
    (conversationId: string) => {
      if (conversationId === activeConversationId) return;
      requestTemporaryChatExit(async () => {
        await useChatStore.getState().selectConversation(conversationId);
      });
    },
    [activeConversationId, requestTemporaryChatExit],
  );

  const {
    activeDropZone,
    handleWorkspaceDragEnter,
    handleWorkspaceDragOver,
    handleWorkspaceDragLeave,
    handleWorkspaceDrop,
  } = useAppDragDrop({
    isRightPanelOpen,
    rightPanelShellRef,
    workspaceDropRef,
    onOpenConversationInPanel: handleOpenConversationInPanel,
    onOpenConversationInPrimary: handleOpenConversationInPrimary,
  });

  const handleWorkspaceResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const container = workspaceDropRef.current;
    if (!container) return;

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      if (bounds.width <= 0) return;
      const nextRatio = clampWorkspacePanelRatio(((moveEvent.clientX - bounds.left) / bounds.width) * 100);
      setWorkspaceRatio(nextRatio);
    };

    const handlePointerUp = (upEvent: PointerEvent) => {
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
  };

  const handleWorkspaceResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowLeft' ? -5 : 5;
    setWorkspaceRatio((current) => {
      const next = clampWorkspacePanelRatio(current + delta);
      writeWorkspacePanelRatio(next);
      return next;
    });
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
      {themeSaveFailed ? (
        <p
          className="absolute right-4 top-14 z-overlay rounded-control border border-destructive bg-raised px-3 py-2 text-sm text-destructive"
          role="alert"
        >
          {t('themeSaveFailed')}
        </p>
      ) : null}
      <TitleBar
        canGoBack={surfaceHistory.index > 0}
        canGoForward={surfaceHistory.index < surfaceHistory.surfaces.length - 1}
        isSidebarOpen={isSidebarOpen}
        onGoBack={() => moveInHistory(-1)}
        onGoForward={() => moveInHistory(1)}
        onOpenCommandPalette={() => setIsCommandPaletteOpen(true)}
        onSelectConversation={openSearchConversation}
        onToggleSidebar={activeSurface === 'chats' ? () => setIsSidebarOpen((open) => !open) : undefined}
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
            <div className="flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden">
              <div
                aria-hidden={!isSidebarOpen || undefined}
                className="conversation-sidebar-shell"
                data-open={isSidebarOpen ? 'true' : 'false'}
                inert={!isSidebarOpen}
              >
                <Suspense
                  fallback={
                    <div className="flex h-full min-h-0 w-full items-center justify-center px-4 py-8">
                      <p aria-live="polite" className="text-center text-xs text-secondary-text" role="status">
                        {t('conversationsLoading')}
                      </p>
                    </div>
                  }
                >
                  <ConversationSidebar
                    onCloseMobile={() => setIsSidebarOpen(false)}
                    onManageAgents={openAgentLibrary}
                    onNewAgentChat={startWorkspaceAgentChat}
                    onNewChat={startWorkspaceChat}
                    onRequestTemporaryExit={requestTemporaryChatExit}
                    onStartTemporaryChat={startWorkspaceTemporaryChat}
                  />
                </Suspense>
              </div>
              <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                <section
                  aria-label={t('composerDropTargetLabel')}
                  className="chat-workspace-layout relative flex min-h-0 min-w-0 flex-1 overflow-hidden"
                  onDragEnter={handleWorkspaceDragEnter}
                  onDragLeave={handleWorkspaceDragLeave}
                  onDragOver={handleWorkspaceDragOver}
                  onDrop={handleWorkspaceDrop}
                  ref={workspaceDropRef}
                >
                  <div
                    className="chat-primary-pane relative flex min-h-0 min-w-0 flex-col overflow-hidden"
                    style={isRightPanelOpen ? { flex: `${workspaceRatio} 1 0%` } : { flex: '1 1 0%' }}
                  >
                    <Suspense
                      fallback={
                        <p
                          aria-live="polite"
                          className="flex min-h-0 flex-1 items-center justify-center px-6 py-8 text-sm text-secondary-text"
                          role="status"
                        >
                          {t('messageHistoryLoading')}
                        </p>
                      }
                    >
                      <ConversationPane
                        header={({ isRightPanelOpen, toggleRightPanel }) => (
                          <ConversationActionsHeader
                            activeConversationId={activeConversationId}
                            conversations={conversations}
                            isRightPanelOpen={isRightPanelOpen}
                            onToggleRightPanel={toggleRightPanel}
                          />
                        )}
                        hideInternalRightPanel
                        isLargeText={appearance.conversationTextSize === 'large'}
                        isRightPanelOpen={isRightPanelOpen}
                        notificationFeedbackKey={notificationFeedback}
                        onDismissNotificationFeedback={() => setNotificationFeedback(null)}
                        onOpenRightPanel={(tab) => handleOpenRightPanelForConversation(activeConversationId, tab)}
                        onOpenWorkspaceFile={(file) =>
                          handleOpenWorkspaceFileForConversation(activeConversationId, file)
                        }
                        onOpenSettings={() => navigate('settings')}
                        onQuestionFocusHandled={clearQuestionFocusTarget}
                        onRequestTemporaryExit={requestTemporaryChatExit}
                        onToggleRightPanel={handleTogglePrimaryRightPanel}
                        questionFocusTarget={questionFocusTarget}
                      />
                    </Suspense>
                  </div>

                  {isRightPanelOpen ? (
                    <>
                      {/* biome-ignore lint/a11y/useSemanticElements: The adjustable primary and workspace widths use the ARIA window splitter pattern. */}
                      <div
                        aria-label={t('resizeWorkspacePanel')}
                        aria-orientation="vertical"
                        aria-valuemax={70}
                        aria-valuemin={30}
                        aria-valuenow={workspaceRatio}
                        className="chat-workspace-panel-separator group relative z-20 flex w-1.5 shrink-0 cursor-col-resize items-center justify-center bg-border/60 transition-colors hover:bg-accent/50 focus-visible:bg-accent focus-visible:outline-none"
                        onKeyDown={handleWorkspaceResizeKeyDown}
                        onPointerDown={handleWorkspaceResizePointerDown}
                        role="separator"
                        tabIndex={0}
                      >
                        <span className="h-8 w-0.5 rounded-full bg-secondary-icon/50 transition-colors group-hover:bg-accent" />
                      </div>
                    </>
                  ) : null}

                  <div
                    aria-hidden={!isRightPanelOpen || undefined}
                    className="chat-right-panel-shell"
                    data-open={isRightPanelOpen ? 'true' : 'false'}
                    inert={!isRightPanelOpen}
                    ref={rightPanelShellRef}
                    style={isRightPanelOpen ? { flex: `${100 - workspaceRatio} 1 0%` } : undefined}
                  >
                    {isRightPanelOpen || workspaceFileRequest !== null || workspacePanelRequest !== null ? (
                      <Suspense fallback={null}>
                        <ChatFileChangesPanel
                          conversationId={activeConversationId}
                          conversations={conversations}
                          conversationListReady={serviceStatus === 'ready'}
                          fileRequest={workspaceFileRequest?.request ?? null}
                          initialTab={null}
                          panelRequest={workspacePanelRequest}
                          renderConversation={(panelConversationId) => (
                            <ConversationPane
                              conversationId={panelConversationId}
                              key={panelConversationId}
                              header={() => (
                                <ConversationActionsHeader
                                  activeConversationId={panelConversationId}
                                  conversations={conversations}
                                  isPanelConversation
                                />
                              )}
                              hideInternalRightPanel
                              isLargeText={appearance.conversationTextSize === 'large'}
                              isRightPanelOpen={isRightPanelOpen}
                              notificationFeedbackKey={null}
                              onDismissNotificationFeedback={() => undefined}
                              onOpenRightPanel={(tab) => handleOpenRightPanelForConversation(panelConversationId, tab)}
                              onOpenWorkspaceFile={(file) =>
                                handleOpenWorkspaceFileForConversation(panelConversationId, file)
                              }
                              onOpenSettings={() => navigate('settings')}
                              onQuestionFocusHandled={() => undefined}
                              onRequestTemporaryExit={requestTemporaryChatExit}
                              onToggleRightPanel={() => setIsRightPanelOpen(false)}
                              questionFocusTarget={null}
                            />
                          )}
                        />
                      </Suspense>
                    ) : null}
                  </div>

                  {activeDropZone !== null ? (
                    <div
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center bg-background/55 p-6 backdrop-blur-overlay"
                    >
                      <div className="flex max-w-sm flex-col items-center justify-center gap-3 rounded-card border-2 border-dashed border-accent bg-accent/15 p-8 text-center text-text shadow-lg animate-in zoom-in-95 duration-150">
                        {activeDropZone === 'panel' ? (
                          <>
                            <PanelRight aria-hidden="true" className="text-accent" size={32} />
                            <div className="space-y-1">
                              <p className="text-base font-semibold">{t('dropConversationPanelTitle')}</p>
                              <p className="text-xs text-secondary-text">{t('dropConversationPanelHint')}</p>
                            </div>
                          </>
                        ) : (
                          <>
                            <MessageSquare aria-hidden="true" className="text-accent" size={32} />
                            <div className="space-y-1">
                              <p className="text-base font-semibold">{t('dropConversationPrimaryTitle')}</p>
                              <p className="text-xs text-secondary-text">{t('dropConversationPrimaryHint')}</p>
                            </div>
                          </>
                        )}
                      </div>
                    </div>
                  ) : null}
                </section>
              </div>
            </div>
          ) : activeSurface === 'models' ? (
            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-surface">
              <Suspense
                fallback={
                  <p aria-live="polite" className="px-6 py-8 text-sm text-secondary-text">
                    {t('modelsLoading')}
                  </p>
                }
              >
                <LibraryPage
                  initialTab={libraryTabRequest.tab}
                  initialTabRequestId={libraryTabRequest.requestId}
                  onUsePrompt={(draft) => {
                    setPendingComposerDraft(draft);
                    navigateSurface('chats');
                  }}
                />
              </Suspense>
            </div>
          ) : activeSurface === 'design' ? (
            <div className="min-w-0 flex-1 overflow-hidden bg-surface">
              <Suspense
                fallback={
                  <p aria-live="polite" className="px-6 py-8 text-sm text-secondary-text">
                    {t('designLoading')}
                  </p>
                }
              >
                <DesignPage onOpenSettings={() => navigate('settings')} />
              </Suspense>
            </div>
          ) : activeSurface === 'security' ? (
            <Suspense
              fallback={
                <p aria-live="polite" className="px-6 py-8 text-sm text-secondary-text" role="status">
                  {t('securityScansLoading')}
                </p>
              }
            >
              <SecurityScanPage />
            </Suspense>
          ) : (
            <div className="min-w-0 flex-1 overflow-hidden">
              <Suspense
                fallback={
                  <p aria-live="polite" className="px-6 py-8 text-sm text-secondary-text">
                    {t('settingsSectionLoading')}
                  </p>
                }
              >
                <SettingsPage
                  appearance={appearance}
                  onOpenConversation={openStatisticsConversation}
                  onAppearanceChange={(updated) => setAppearance((current) => ({ ...current, ...updated }))}
                  onThemeChange={changeThemePreference}
                  sectionRequest={settingsSectionRequest}
                  theme={theme}
                />
              </Suspense>
            </div>
          )}
        </main>
      </div>
      <footer className="app-shell-chrome app-shell-footer flex select-none items-center justify-between gap-2 px-2 py-1 text-xs font-medium text-secondary-text">
        <span className="flex items-center gap-2">
          <a
            aria-label={t('openChatGitHubRepository')}
            className="inline-flex items-center rounded-xs hover:text-text focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
            href="https://github.com/alztrk/openchat"
            rel="noreferrer"
            target="_blank"
            title={t('github')}
          >
            <Github aria-hidden="true" size={14} />
            <span className="ml-1">{t('github')}</span>
          </a>
          <span>{appVersion ? `v${appVersion}` : null}</span>
        </span>
        <span className="flex select-none items-center gap-1.5">
          <BrandMark size="footer" />
          <span>{t('appTitle')}</span>
        </span>
      </footer>
      <AppModals
        activeSurface={activeSurface}
        closeTemporaryChatExitDialog={closeTemporaryChatExitDialog}
        discardTemporaryChatAndContinue={discardTemporaryChatAndContinue}
        isCommandPaletteOpen={isCommandPaletteOpen}
        isDark={isDark}
        isSavingTemporaryChat={isSavingTemporaryChat}
        navigate={navigate}
        navigateSurface={navigateSurface}
        openAgentLibrary={openAgentLibrary}
        openSearchConversation={openSearchConversation}
        requestTemporaryChatExit={requestTemporaryChatExit}
        saveTemporaryChatAndContinue={saveTemporaryChatAndContinue}
        setIsCommandPaletteOpen={setIsCommandPaletteOpen}
        setSettingsSectionRequest={setSettingsSectionRequest}
        startWorkspaceAgentChat={startWorkspaceAgentChat}
        startWorkspaceChat={startWorkspaceChat}
        temporaryExitIntent={temporaryExitIntent}
        toggleTheme={toggleTheme}
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
