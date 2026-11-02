import { isTerminalChatGoalStatus, parseChatGoal } from '@features/chat/data/chatGoals';
import { localServiceClient } from '@shared/lib/platform/local-service';
import { isObject } from './chatStoreUtils';
import type { ChatState } from './chatStore';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

let goalChangeGeneration = 0;
let goalRefreshGeneration = 0;

export function createGoalsActions(set: StoreSet, get: StoreGet) {
  return {
    async refreshInterruptedRun(conversationId: string): Promise<void> {
      if (get().activeConversationId !== conversationId) return;
      try {
        const response = await localServiceClient.call('chat.runs.list', { limit: 50 });
        if (!Array.isArray(response.runs)) throw new Error('The saved run list is invalid.');
        const run = response.runs.find(
          (candidate) => isInterruptedRunSummary(candidate) && candidate.conversationId === conversationId,
        );
        if (get().activeConversationId !== conversationId) return;
        set({ interruptedRun: run ? { conversationId, runId: run.runId } : null });
      } catch (cause) {
        globalThis.reportError(new Error('The interrupted response could not be checked.', { cause }));
        if (get().activeConversationId === conversationId) set({ interruptedRun: null });
      }
    },

    async resumeInterruptedGoalsAfterRestart(): Promise<void> {
      try {
        const response = await localServiceClient.call('chat.runs.list', { limit: 50 });
        if (!Array.isArray(response.runs)) throw new Error('The saved run list is invalid.');

        const interruptedRuns = response.runs.flatMap((run) =>
          isInterruptedRunSummary(run) ? [{ runId: run.runId, conversationId: run.conversationId }] : [],
        );
        const resumableRuns = (
          await Promise.all(
            interruptedRuns.map(async (run) => {
              try {
                const goalResponse = await localServiceClient.call('chat.goal.active', {
                  conversationId: run.conversationId,
                });
                if (!('goal' in goalResponse)) throw new Error('The active goal response is invalid.');
                const goal = parseChatGoal(goalResponse.goal);
                return goal?.runId === run.runId && goal.status === 'interrupted' ? run : null;
              } catch (cause) {
                globalThis.reportError(new Error('An interrupted goal could not be checked for recovery.', { cause }));
                return null;
              }
            }),
          )
        ).filter((run): run is { runId: string; conversationId: string } => run !== null);

        const resumeResults = await Promise.allSettled(
          resumableRuns.map((run) =>
            get().sendMessage('', [], run.runId, undefined, { conversationId: run.conversationId }),
          ),
        );
        for (const result of resumeResults) {
          if (result.status === 'rejected') {
            globalThis.reportError(
              new Error('An interrupted goal could not be resumed after restart.', { cause: result.reason }),
            );
          } else if (!result.value) {
            globalThis.reportError(new Error('An interrupted goal could not be started after restart.'));
          }
        }
      } catch (cause) {
        globalThis.reportError(new Error('Interrupted goals could not be recovered after restart.', { cause }));
      }
    },

    async refreshActiveGoal(conversationId: string): Promise<void> {
      if (get().activeConversationId !== conversationId) return;
      const generation = ++goalRefreshGeneration;
      set({ goalStatus: 'loading' });
      try {
        const response = await localServiceClient.call('chat.goal.active', { conversationId });
        if (!('goal' in response)) throw new Error('The active goal response is invalid.');
        const goal = parseChatGoal(response.goal);
        if (goal && goal.conversationId !== conversationId) throw new Error('The active goal belongs to another chat.');
        if (get().activeConversationId !== conversationId || generation !== goalRefreshGeneration) return;
        set({
          activeGoal: goal && !isTerminalChatGoalStatus(goal.status) ? goal : null,
          goalStatus: 'ready',
        });
      } catch {
        if (get().activeConversationId === conversationId && generation === goalRefreshGeneration) {
          set({ activeGoal: null, goalStatus: 'error' });
        }
      }
    },

    async pauseActiveGoal(reason: 'user_paused' | 'budget_exceeded' = 'user_paused'): Promise<void> {
      const current = get();
      const goal = current.activeGoal;
      if (goal?.status !== 'running' || current.isChangingGoal) return;
      const generation = ++goalChangeGeneration;
      set({ isChangingGoal: true, goalStatus: 'ready' });
      try {
        await localServiceClient.call('chat.goal.pause', {
          conversationId: goal.conversationId,
          runId: goal.runId,
          reason,
        });
        if (
          generation !== goalChangeGeneration ||
          get().activeConversationId !== goal.conversationId ||
          get().activeGoal?.runId !== goal.runId
        )
          return;
        await current.activeOperationsByConversation[goal.conversationId]?.cancel();
        await get().refreshActiveGoal(goal.conversationId);
      } catch {
        if (generation === goalChangeGeneration && get().activeConversationId === goal.conversationId) {
          set({ goalStatus: 'error' });
        }
      } finally {
        if (generation === goalChangeGeneration) set({ isChangingGoal: false });
      }
    },

    async stopActiveGoal(): Promise<void> {
      const current = get();
      const goal = current.activeGoal;
      if (!goal || current.isChangingGoal) return;
      const generation = ++goalChangeGeneration;
      set({ isChangingGoal: true, goalStatus: 'ready' });
      try {
        await localServiceClient.call('chat.goal.stop', {
          conversationId: goal.conversationId,
          runId: goal.runId,
        });
        if (
          generation !== goalChangeGeneration ||
          get().activeConversationId !== goal.conversationId ||
          get().activeGoal?.runId !== goal.runId
        )
          return;
        await current.activeOperationsByConversation[goal.conversationId]?.cancel();
        await get().refreshActiveGoal(goal.conversationId);
      } catch {
        if (generation === goalChangeGeneration && get().activeConversationId === goal.conversationId) {
          set({ goalStatus: 'error' });
        }
      } finally {
        if (generation === goalChangeGeneration) set({ isChangingGoal: false });
      }
    },
  };
}

function isInterruptedRunSummary(value: unknown): value is { runId: string; conversationId: string } {
  return (
    isObject(value) &&
    typeof value.runId === 'string' &&
    value.runId.length > 0 &&
    value.runId.length <= 128 &&
    typeof value.conversationId === 'string' &&
    value.conversationId.length > 0 &&
    value.conversationId.length <= 128 &&
    value.status === 'interrupted'
  );
}
