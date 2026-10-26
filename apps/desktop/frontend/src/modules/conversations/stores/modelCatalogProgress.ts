import type { ChatModel } from '@/modules/providers/data/chatModels';
import type { ModelCatalogBatch } from '@/modules/providers/data/modelCatalogLoader';

export type ModelCatalogLoadResult<T> = { status: 'fulfilled'; value: T } | { status: 'rejected'; reason: unknown };

export interface ModelCatalogProgress {
  chatGpt: ModelCatalogLoadResult<ModelCatalogBatch> | null;
  local: ModelCatalogLoadResult<ChatModel[]> | null;
  apiKey: ModelCatalogLoadResult<ModelCatalogBatch> | null;
  allSettled: boolean;
}

/** Publishes each completed model catalog while preserving the combined completion state. */
export async function loadModelCatalogsProgressively(
  loads: {
    chatGpt: Promise<ModelCatalogBatch>;
    local: Promise<ChatModel[]>;
    apiKey: (onProgress: (batch: ModelCatalogBatch) => void) => Promise<ModelCatalogBatch>;
  },
  onProgress: (progress: ModelCatalogProgress) => void,
): Promise<void> {
  let chatGpt: ModelCatalogProgress['chatGpt'] = null;
  let local: ModelCatalogProgress['local'] = null;
  let apiKey: ModelCatalogProgress['apiKey'] = null;
  let chatGptSettled = false;
  let localSettled = false;
  let apiKeySettled = false;

  const publish = () => {
    onProgress({
      chatGpt,
      local,
      apiKey,
      allSettled: chatGptSettled && localSettled && apiKeySettled,
    });
  };

  const observe = <T>(promise: Promise<T>, receive: (result: ModelCatalogLoadResult<T>) => void) =>
    promise.then(
      (value) => {
        receive({ status: 'fulfilled', value });
        publish();
      },
      (reason: unknown) => {
        receive({ status: 'rejected', reason });
        publish();
      },
    );

  await Promise.all([
    observe(loads.chatGpt, (result) => {
      chatGpt = result;
      chatGptSettled = true;
    }),
    observe(loads.local, (result) => {
      local = result;
      localSettled = true;
    }),
    observe(
      loads.apiKey((batch) => {
        apiKey = { status: 'fulfilled', value: batch };
        publish();
      }),
      (result) => {
        apiKey = result;
        apiKeySettled = true;
      },
    ),
  ]);
}
