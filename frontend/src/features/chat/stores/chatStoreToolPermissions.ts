import type { StoreApi } from 'zustand';

import { LocalServiceError, type JsonObject, type LocalServiceClient, type ServiceEvent } from '@/shared/services/LocalServiceClient';
import type { ChatState, ChatToolPermissionRequest } from './chatStore';

type SetState = StoreApi<ChatState>['setState'];
type GetState = StoreApi<ChatState>['getState'];

export function handleToolPermissionEvent(event: ServiceEvent, set: SetState, get: GetState): void {
  if (event.name !== 'chat.tool.permission.requested') return;
  const request = parseToolPermissionRequest(event.data);
  if (!request) {
    set({ toolPermissionError: 'toolPermissionResponseFailed' });
    void get().stopResponse().catch(() => set({ toolPermissionError: 'toolPermissionResponseFailed' }));
    return;
  }

  const pending = get().pendingToolPermissionRequest;
  if (pending && pending.id !== request.id && !get().isRespondingToToolPermission) {
    set({ toolPermissionError: 'toolPermissionResponseFailed' });
    void get().stopResponse().catch(() => set({ toolPermissionError: 'toolPermissionResponseFailed' }));
    return;
  }
  if (pending?.id === request.id) return;
  set({
    pendingToolPermissionRequest: request,
    isRespondingToToolPermission: false,
    toolPermissionError: null,
  });
}

export function createRespondToToolPermissionAction(
  set: SetState,
  get: GetState,
  localServiceClient: LocalServiceClient,
): ChatState['respondToToolPermission'] {
  return async (approved) => {
    const request = get().pendingToolPermissionRequest;
    if (!request || get().isRespondingToToolPermission) return;
    set({ isRespondingToToolPermission: true, toolPermissionError: null });
    try {
      await localServiceClient.call('chat.tool.permission.respond', {
        approvalRequestId: request.id,
        approved,
      });
      if (get().pendingToolPermissionRequest?.id === request.id) {
        set({
          pendingToolPermissionRequest: null,
          isRespondingToToolPermission: false,
          toolPermissionError: null,
        });
      }
    } catch (error) {
      if (get().pendingToolPermissionRequest?.id !== request.id) return;
      if (error instanceof LocalServiceError && error.code === 'tool_permission_request_unavailable') {
        set({
          pendingToolPermissionRequest: null,
          isRespondingToToolPermission: false,
          toolPermissionError: 'toolPermissionRequestExpired',
        });
        return;
      }
      set({ isRespondingToToolPermission: false, toolPermissionError: 'toolPermissionResponseFailed' });
    }
  };
}

function parseToolPermissionRequest(value: JsonObject): ChatToolPermissionRequest | null {
  const id = value.approvalRequestId;
  const toolName = value.toolName;
  const targetPath = value.targetPath;
  const args = value.arguments;
  if (
    typeof id !== 'string' || id.length === 0 ||
    typeof toolName !== 'string' || toolName.length === 0 ||
    typeof targetPath !== 'string' || targetPath.length === 0 ||
    !isJsonObject(args)
  ) return null;
  return { id, toolName, targetPath, arguments: args };
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
