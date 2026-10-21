import { Columns2, Github, MessageSquare } from 'lucide-react';
import {
  type DragEvent as ReactDragEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  lazy,
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
import {
  ChatFileChangesPanel,
  type RightWorkspaceTab,
} from '@/modules/conversations/components/ChatFileChangesPanel';
import { ConversationPane } from '@/modules/conversations/components/ConversationPane';
import { ConversationSidebar } from '@/modules/conversations/components/ConversationSidebar';
import { TemporaryChatExitDialog } from '@/modules/conversations/components/TemporaryChatExitDialog';
import { parseAgentQuestionGroup } from '@/modules/conversations/data/agentQuestions';
import { getMessages } from '@/modules/conversations/data/chatStorage';
import {
  clampSplitViewRatio,
  getSidebarDraggingConversationId,
  readSplitViewRatio,
  setSidebarDraggingConversationId,
  writeSplitViewRatio,
} from '@/modules/conversations/data/sidebarPreferences';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import {
  resetNotificationWindowFocus,
  setNotificationWindowFocus,
} from '@/modules/notifications/data/notificationRuntime';
import {
  OAUTH_PLAN_CHECK_INTERVAL_MS,
  refreshOAuthPlans,
  shouldRefreshOAuthPlansOnFocus,
} from '@/modules/providers/data/oauthPlanMonitor';
import type { ThemePreference } from '@/modules/settings/components/GeneralSettingsSection';
import {
  type AppearancePreferences,
  getAppearanceCssValues,
  readAppearancePreferences,
} from '@/modules/settings/data/appearancePreferences';
import { localServiceClient } from '@/platform/local-service';
import { unifiedStorage } from '@/platform/storage/unifiedStorage';
import { getCurrentWindow, getVersion, invoke, isTauri, listen } from '@/platform/tauri';
import { BrandMark } from '@/shell/BrandMark';
import { ConversationActionsHeader } from '@/shell/ConversationActionsHeader';
import { type AppSurface, NavigationRail } from '@/shell/NavigationRail';
import { TitleBar } from '@/shell/TitleBar';

const LibraryPage = lazy(() =>
  import('@/modules/model-library/components/LibraryPage').then((module) => ({ default: module.LibraryPage })),
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
  const [systemIsDark, setSystemIsDark] = useState(() => window.matchMedia('(prefers-color-scheme: dark)').matches);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [splitConversationId, setSplitConversationId] = useState<string | null>(null);
  const [splitRatio, setSplitRatio] = useState<number>(readSplitViewRatio);
  const [draggingConversationId, setDraggingConversationId] = useState<string | null>(null);
  const [activeDropZone, setActiveDropZone] = useState<'primary' | 'split' | null>(null);
  const workspaceDropRef = useRef<HTMLDivElement>(null);
  const surfaceHeadingRef = useRef<HTMLElement>(null);
  const lastInteractionMethod = useRef<'keyboard' | 'pointer'>('pointer');
  const initializeChat = useChatStore((state) => state.initialize);
  const serviceStatus = useChatStore((state) => state.serviceStatus);
  const conversations = useChatStore((state) => state.conversations);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const startNewChat = useChatStore((state) => state.startNewChat);
  const startTemporaryChat = useChatStore((state) => state.startTemporaryChat);
  const temporaryChat = useChatStore((state) => state.temporaryChat);
  const temporaryChatMessageCount = useChatStore((state) => state.messages.length);
  const temporaryChatErrorCode = useChatStore((state) => state.errorCode);
  const isDark = theme === 'dark' || (theme === 'system' && systemIsDark);

  useEffect(() => {
    if (conversations.length === 0) return;
    const validIds = new Set(conversations.filter((c) => !c.isArchived).map((c) => c.id));
    setSplitConversationId((current) => (current && validIds.has(current) ? current : null));
  }, [conversations]);

  useEffect(() => {
    const onDragState = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail = event.detail as { conversationId?: unknown; dragging?: unknown } | null;
      if (detail && typeof detail.conversationId === 'string' && detail.dragging === true) {
        setDraggingConversationId(detail.conversationId);
      } else {
        setDraggingConversationId(null);
        setActiveDropZone(null);
      }
    };
    const onOpenSplit = (event: Event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail = event.detail as { conversationId?: unknown } | null;
      const targetId = detail && typeof detail.conversationId === 'string' ? detail.conversationId : null;
      if (!targetId) return;
      setSplitConversationId(targetId);
    };
    window.addEventListener('openchat-conversation-drag-state', onDragState);
    window.addEventListener('openchat-open-split-conversation', onOpenSplit);
    return () => {
      window.removeEventListener('openchat-conversation-drag-state', onDragState);
      window.removeEventListener('openchat-open-split-conversation', onOpenSplit);
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
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
        const target = event.target;
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
        event.preventDefault();
        setIsSidebarOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

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
        navigateSurface(surface);
      });
    },
    [activeSurface, navigateSurface, requestTemporaryChatExit],
  );

  const startWorkspaceChat = () => {
    requestTemporaryChatExit(() => {
      navigateSurface('chats');
      startNewChat();
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
      unifiedStorage.setItem('openchat.theme', nextTheme);
    } catch {
      // The selected theme still applies for this session when storage is unavailable.
    }
    changeThemePreference(nextTheme);
  };

  const [activeRightPanelConversationId, setActiveRightPanelConversationId] = useState<string | null>(null);
  const [rightPanelTab, setRightPanelTab] = useState<RightWorkspaceTab>('changes');

  const handleTogglePrimaryRightPanel = useCallback(() => {
    if (!activeConversationId) return;
    setActiveRightPanelConversationId((current) =>
      current === activeConversationId ? null : activeConversationId,
    );
  }, [activeConversationId]);

  const handleToggleSecondaryRightPanel = useCallback(() => {
    if (!splitConversationId) return;
    setActiveRightPanelConversationId((current) =>
      current === splitConversationId ? null : splitConversationId,
    );
  }, [splitConversationId]);

  const handleOpenRightPanelForConversation = useCallback(
    (conversationId: string | null, tab: RightWorkspaceTab = 'changes') => {
      if (!conversationId) return;
      setRightPanelTab(tab);
      setActiveRightPanelConversationId(conversationId);
    },
    [],
  );

  const handleToggleSplitView = useCallback(() => {
    if (splitConversationId) {
      if (activeRightPanelConversationId === splitConversationId) {
        setActiveRightPanelConversationId(null);
      }
      setSplitConversationId(null);
      return;
    }
    setSplitConversationId('__empty__');
  }, [activeRightPanelConversationId, splitConversationId]);

  const handleCloseSecondarySplit = useCallback(() => {
    if (activeRightPanelConversationId === splitConversationId) {
      setActiveRightPanelConversationId(null);
    }
    setSplitConversationId(null);
  }, [activeRightPanelConversationId, splitConversationId]);

  const handleSwapSplitPanes = useCallback(() => {
    if (!splitConversationId) return;
    const previousPrimary = activeConversationId;
    const nextPrimary = splitConversationId;
    requestTemporaryChatExit(async () => {
      setSplitConversationId(previousPrimary ?? nextPrimary);
      await useChatStore.getState().selectConversation(nextPrimary);
      setActiveRightPanelConversationId((current) => {
        if (current === previousPrimary) return nextPrimary;
        if (current === nextPrimary) return previousPrimary;
        return current;
      });
    });
  }, [activeConversationId, requestTemporaryChatExit, splitConversationId]);

  const handlePromoteSplitToPrimary = useCallback(() => {
    if (!splitConversationId) return;
    const targetId = splitConversationId;
    setSplitConversationId(null);
    requestTemporaryChatExit(async () => {
      await useChatStore.getState().selectConversation(targetId);
    });
  }, [requestTemporaryChatExit, splitConversationId]);

  const handleOpenOrSplitConversation = useCallback(
    (draggedId: string) => {
      if (!draggedId) return;
      if (!activeConversationId) {
        requestTemporaryChatExit(async () => {
          await useChatStore.getState().selectConversation(draggedId);
        });
        return;
      }
      if (draggedId !== activeConversationId) {
        setSplitConversationId(draggedId);
      }
    },
    [activeConversationId, requestTemporaryChatExit],
  );

  const isConversationDragEvent = (event: ReactDragEvent<HTMLElement> | DragEvent): boolean => {
    if (draggingConversationId !== null) return true;
    if (getSidebarDraggingConversationId() !== null) return true;
    const types = Array.from(event.dataTransfer?.types ?? []);
    return (
      types.includes('application/x-openchat-conversation') ||
      types.includes('text/plain')
    );
  };

  const workspaceDragDepthRef = useRef(0);

  useEffect(() => {
    const handleGlobalDragOver = (event: DragEvent) => {
      const isConv =
        getSidebarDraggingConversationId() !== null ||
        Array.from(event.dataTransfer?.types ?? []).includes('application/x-openchat-conversation');
      if (isConv) {
        event.preventDefault();
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = 'copy';
        }
      }
    };

    const handleGlobalDragEnd = () => {
      workspaceDragDepthRef.current = 0;
      setActiveDropZone(null);
      setDraggingConversationId(null);
      setSidebarDraggingConversationId(null);
    };

    window.addEventListener('dragover', handleGlobalDragOver);
    window.addEventListener('dragend', handleGlobalDragEnd);
    window.addEventListener('drop', handleGlobalDragEnd);
    return () => {
      window.removeEventListener('dragover', handleGlobalDragOver);
      window.removeEventListener('dragend', handleGlobalDragEnd);
      window.removeEventListener('drop', handleGlobalDragEnd);
    };
  }, []);

  const handleWorkspaceDragEnter = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    workspaceDragDepthRef.current += 1;
    const targetZone = activeConversationId ? 'split' : 'primary';
    setActiveDropZone(targetZone);
  };

  const handleWorkspaceDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
    const targetZone = activeConversationId ? 'split' : 'primary';
    if (activeDropZone !== targetZone) {
      setActiveDropZone(targetZone);
    }
  };

  const handleWorkspaceDragLeave = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    workspaceDragDepthRef.current = Math.max(0, workspaceDragDepthRef.current - 1);
    if (workspaceDragDepthRef.current === 0) {
      setActiveDropZone(null);
    }
  };

  const handleWorkspaceDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!isConversationDragEvent(event)) return;
    event.preventDefault();
    event.stopPropagation();
    workspaceDragDepthRef.current = 0;
    setActiveDropZone(null);

    const customData = event.dataTransfer.getData('application/x-openchat-conversation');
    const inMemoryId = draggingConversationId || getSidebarDraggingConversationId();
    const plainText = event.dataTransfer.getData('text/plain');
    const matchingPlainId = conversations.some((c) => c.id === plainText) ? plainText : '';
    const draggedId = customData || inMemoryId || matchingPlainId;

    setDraggingConversationId(null);
    setSidebarDraggingConversationId(null);
    if (draggedId) {
      handleOpenOrSplitConversation(draggedId);
    }
  };

  const handleSplitResizePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const container = workspaceDropRef.current;
    if (!container) return;

    const handlePointerMove = (moveEvent: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      if (bounds.width <= 0) return;
      const nextRatio = clampSplitViewRatio(((moveEvent.clientX - bounds.left) / bounds.width) * 100);
      setSplitRatio(nextRatio);
    };

    const handlePointerUp = (upEvent: PointerEvent) => {
      const bounds = container.getBoundingClientRect();
      if (bounds.width > 0) {
        const finalRatio = clampSplitViewRatio(((upEvent.clientX - bounds.left) / bounds.width) * 100);
        setSplitRatio(finalRatio);
        writeSplitViewRatio(finalRatio);
      }
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);
  };

  const handleSplitResizeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowLeft' ? -5 : 5;
    setSplitRatio((current) => {
      const next = clampSplitViewRatio(current + delta);
      writeSplitViewRatio(next);
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
      <TitleBar
        canGoBack={surfaceHistory.index > 0}
        canGoForward={surfaceHistory.index < surfaceHistory.surfaces.length - 1}
        isSidebarOpen={isSidebarOpen}
        onGoBack={() => moveInHistory(-1)}
        onGoForward={() => moveInHistory(1)}
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
                <ConversationSidebar
                  onCloseMobile={() => setIsSidebarOpen(false)}
                  onNewChat={startWorkspaceChat}
                  onRequestTemporaryExit={requestTemporaryChatExit}
                  onStartTemporaryChat={startWorkspaceTemporaryChat}
                />
              </div>
              <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                <div
                  className="relative flex min-h-0 min-w-0 flex-1 overflow-hidden"
                  onDragEnter={handleWorkspaceDragEnter}
                  onDragLeave={handleWorkspaceDragLeave}
                  onDragOver={handleWorkspaceDragOver}
                  onDrop={handleWorkspaceDrop}
                  ref={workspaceDropRef}
                >
                  <div
                    className="relative flex min-h-0 min-w-0 flex-col overflow-hidden"
                    style={
                      splitConversationId && !temporaryChat
                        ? { width: `${splitRatio}%`, flexShrink: 0 }
                        : { flex: '1 1 0%' }
                    }
                  >
                    <ConversationPane
                      header={({ isRightPanelOpen, toggleRightPanel }) => (
                        <ConversationActionsHeader
                          activeConversationId={activeConversationId}
                          conversations={conversations}
                          isRightPanelOpen={isRightPanelOpen}
                          onToggleRightPanel={toggleRightPanel}
                          onToggleSplitView={handleToggleSplitView}
                          splitConversationId={splitConversationId}
                        />
                      )}
                      hideInternalRightPanel
                      isLargeText={appearance.conversationTextSize === 'large'}
                      isRightPanelOpen={
                        activeConversationId !== null &&
                        activeRightPanelConversationId === activeConversationId
                      }
                      notificationFeedbackKey={notificationFeedback}
                      onDismissNotificationFeedback={() => setNotificationFeedback(null)}
                      onDropConversation={handleOpenOrSplitConversation}
                      onOpenRightPanel={(tab) =>
                        handleOpenRightPanelForConversation(activeConversationId, tab)
                      }
                      onOpenSettings={() => navigate('settings')}
                      onQuestionFocusHandled={clearQuestionFocusTarget}
                      onRequestTemporaryExit={requestTemporaryChatExit}
                      onToggleRightPanel={handleTogglePrimaryRightPanel}
                      questionFocusTarget={questionFocusTarget}
                    />
                  </div>

                  {splitConversationId && !temporaryChat ? (
                    <>
                      <div
                        aria-label={t('resizeSplitView')}
                        aria-orientation="vertical"
                        aria-valuemax={70}
                        aria-valuemin={30}
                        aria-valuenow={splitRatio}
                        className="group relative z-20 flex w-1.5 shrink-0 cursor-col-resize items-center justify-center bg-border/60 transition-colors hover:bg-accent/50 focus-visible:bg-accent focus-visible:outline-none"
                        onKeyDown={handleSplitResizeKeyDown}
                        onPointerDown={handleSplitResizePointerDown}
                        role="separator"
                        tabIndex={0}
                      >
                        <span className="h-8 w-0.5 rounded-full bg-secondary-icon/50 transition-colors group-hover:bg-accent" />
                      </div>
                      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden animate-in fade-in slide-in-from-right-4 duration-200 ease-out">
                        <ConversationPane
                          conversationId={splitConversationId}
                          header={
                            <ConversationActionsHeader
                              activeConversationId={splitConversationId}
                              conversations={conversations}
                              isRightPanelOpen={
                                splitConversationId !== null &&
                                activeRightPanelConversationId === splitConversationId
                              }
                              isSplitSecondary={true}
                              onCloseSplit={handleCloseSecondarySplit}
                              onPromoteToPrimary={handlePromoteSplitToPrimary}
                              onSwapWithPrimary={handleSwapSplitPanes}
                              onToggleRightPanel={handleToggleSecondaryRightPanel}
                            />
                          }
                          hideInternalRightPanel
                          isLargeText={appearance.conversationTextSize === 'large'}
                          isRightPanelOpen={
                            splitConversationId !== null &&
                            activeRightPanelConversationId === splitConversationId
                          }
                          notificationFeedbackKey={null}
                          onDismissNotificationFeedback={() => {}}
                          onDropConversation={handleOpenOrSplitConversation}
                          onOpenRightPanel={(tab) =>
                            handleOpenRightPanelForConversation(splitConversationId, tab)
                          }
                          onOpenSettings={() => navigate('settings')}
                          onQuestionFocusHandled={() => {}}
                          onRequestTemporaryExit={requestTemporaryChatExit}
                          onToggleRightPanel={handleToggleSecondaryRightPanel}
                          questionFocusTarget={null}
                        />
                      </div>
                    </>
                  ) : null}

                  <div
                    aria-hidden={!activeRightPanelConversationId || undefined}
                    className="chat-right-panel-shell"
                    data-open={activeRightPanelConversationId ? 'true' : 'false'}
                    inert={!activeRightPanelConversationId}
                  >
                    {activeRightPanelConversationId ? (
                      <ChatFileChangesPanel
                        conversationId={activeRightPanelConversationId}
                        initialTab={rightPanelTab}
                        key={activeRightPanelConversationId}
                        onClose={() => setActiveRightPanelConversationId(null)}
                      />
                    ) : null}
                  </div>

                  {activeDropZone !== null ? (
                    <div
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center bg-background/55 p-6 backdrop-blur-[2px]"
                    >
                      <div className="flex max-w-sm flex-col items-center justify-center gap-3 rounded-card border-2 border-dashed border-accent bg-accent/15 p-8 text-center text-text shadow-lg animate-in zoom-in-95 duration-150">
                        {activeDropZone === 'split' ? (
                          <>
                            <Columns2 aria-hidden="true" className="text-accent" size={32} />
                            <div className="space-y-1">
                              <p className="text-base font-semibold">{t('dropConversationSplitTitle')}</p>
                              <p className="text-xs text-secondary-text">{t('dropConversationSplitHint')}</p>
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
                </div>
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
                <LibraryPage />
              </Suspense>
            </div>
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
