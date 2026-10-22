export async function refreshConversationListAfterResponse<T>(
  load: () => Promise<T[]>,
  onLoaded: (conversations: T[]) => void,
  onFailure: (error: unknown) => void,
): Promise<T[] | null> {
  try {
    const conversations = await load();
    onLoaded(conversations);
    return conversations;
  } catch (error) {
    onFailure(error);
    return null;
  }
}
