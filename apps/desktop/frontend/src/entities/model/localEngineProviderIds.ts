export const LOCAL_ENGINE_PROVIDER_IDS = ['llama_cpp', 'vllm', 'exllama', 'sglang'] as const;

export function isLocalEngineProviderId(providerId: string): boolean {
  return LOCAL_ENGINE_PROVIDER_IDS.some((id) => id === providerId);
}
