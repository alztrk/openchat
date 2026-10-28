import { isTerminalChatGoalStatus, parseChatGoal } from '@/modules/conversations/data/chatGoals';
import { localServiceClient } from '@/platform/local-service';
import type { ChatState } from './chatStore';

type StoreGet = () => ChatState;
type StoreSet = (partial: Partial<ChatState> | ((state: ChatState) => Partial<ChatState>)) => void;

let goalChangeGeneration = 0;
let goalRefreshGeneration = 0;

export function createGoalsActions(set: StoreSet, get: StoreGet) {
  return {
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
        if (current.activeConversationId === goal.conversationId) await current.activeOperation?.cancel();
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
        if (current.activeConversationId === goal.conversationId) await current.activeOperation?.cancel();
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
