import type { StoreApi } from 'zustand';

import {
  type JsonObject,
  type LocalServiceClient,
  LocalServiceError,
  type ServiceEvent,
} from '@shared/lib/platform/local-service';
import type { ChatState, ChatToolPermissionRequest } from './chatStore';

type SetState = StoreApi<ChatState>['setState'];
type GetState = StoreApi<ChatState>['getState'];

export function handleToolPermissionEvent(
  event: ServiceEvent,
  conversationId: string,
  set: SetState,
  get: GetState,
): void {
  if (event.name !== 'chat.tool.permission.requested') return;
  const parsedRequest = parseToolPermissionRequest(event.data);
  if (!parsedRequest) {
    if (get().activeConversationId === conversationId) {
      set({ toolPermissionError: 'toolPermissionResponseFailed' });
    }
    void get()
      .stopResponse(conversationId)
      .catch(() => {
        if (get().activeConversationId === conversationId) {
          set({ toolPermissionError: 'toolPermissionResponseFailed' });
        }
      });
    return;
  }
  const request = { ...parsedRequest, conversationId };

  if (get().pendingToolPermissionRequests.some((pending) => pending.id === request.id)) return;
  set((current) => ({
    pendingToolPermissionRequests: [...current.pendingToolPermissionRequests, request],
    pendingToolPermissionRequest: request,
    isRespondingToToolPermission: current.respondingToolPermissionIds.length > 0,
    toolPermissionError: null,
  }));
}

export function createRespondToToolPermissionAction(
  set: SetState,
  get: GetState,
  localServiceClient: LocalServiceClient,
): ChatState['respondToToolPermission'] {
  return async (approved, conversationId) => {
    const current = get();
    const targetConversationId = conversationId ?? current.activeConversationId;
    const request = current.pendingToolPermissionRequests.find(
      (pending) => pending.conversationId === targetConversationId,
    );
    if (!request || current.respondingToolPermissionIds.includes(request.id)) return;
    set((state) => ({
      isRespondingToToolPermission: true,
      respondingToolPermissionIds: [...state.respondingToolPermissionIds, request.id],
      toolPermissionError: null,
    }));
    try {
      await localServiceClient.respondToPermission(request.id, approved);
      set((state) => {
        const pendingToolPermissionRequests = state.pendingToolPermissionRequests.filter(
          (pending) => pending.id !== request.id,
        );
        return {
          pendingToolPermissionRequests,
          pendingToolPermissionRequest: pendingToolPermissionRequests[0] ?? null,
          isRespondingToToolPermission: state.respondingToolPermissionIds.some((id) => id !== request.id),
          respondingToolPermissionIds: state.respondingToolPermissionIds.filter((id) => id !== request.id),
          toolPermissionError: null,
        };
      });
    } catch (error) {
      if (!get().pendingToolPermissionRequests.some((pending) => pending.id === request.id)) return;
      if (error instanceof LocalServiceError && error.code === 'tool_permission_request_unavailable') {
        set((state) => {
          const pendingToolPermissionRequests = state.pendingToolPermissionRequests.filter(
            (pending) => pending.id !== request.id,
          );
          return {
            pendingToolPermissionRequests,
            pendingToolPermissionRequest: pendingToolPermissionRequests[0] ?? null,
            isRespondingToToolPermission: state.respondingToolPermissionIds.some((id) => id !== request.id),
            respondingToolPermissionIds: state.respondingToolPermissionIds.filter((id) => id !== request.id),
            toolPermissionError: 'toolPermissionRequestExpired',
          };
        });
        return;
      }
      set((state) => ({
        isRespondingToToolPermission: state.respondingToolPermissionIds.some((id) => id !== request.id),
        respondingToolPermissionIds: state.respondingToolPermissionIds.filter((id) => id !== request.id),
        toolPermissionError: 'toolPermissionResponseFailed',
      }));
    }
  };
}

function parseToolPermissionRequest(value: JsonObject): Omit<ChatToolPermissionRequest, 'conversationId'> | null {
  const id = value.approvalRequestId;
  const toolName = value.toolName;
  const targetPath = value.targetPath;
  const args = value.arguments;
  if (
    typeof id !== 'string' ||
    id.length === 0 ||
    typeof toolName !== 'string' ||
    toolName.length === 0 ||
    typeof targetPath !== 'string' ||
    targetPath.length === 0 ||
    !isJsonObject(args)
  )
    return null;
  return { id, toolName, targetPath, arguments: args };
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
