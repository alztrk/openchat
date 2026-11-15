import type { ChatModel } from '@entities/model';

export function shouldRefreshLocalModelsAfterChatStart(
  eventName: string,
  providerId: string,
  isLocalEngine: (providerId: string) => boolean,
): boolean {
  return eventName === 'chat.started' && isLocalEngine(providerId);
}

export function mergeRefreshedLocalModels(
  currentModels: readonly ChatModel[],
  refreshedLocalModels: readonly ChatModel[],
  isLocalEngine: (providerId: string) => boolean,
): ChatModel[] {
  return [...currentModels.filter((model) => !isLocalEngine(model.providerId)), ...refreshedLocalModels];
}
