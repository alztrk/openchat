const draftsByConversation = new Map<string, string>();

export function readConversationComposerDraft(conversationId: string): string | null {
  return draftsByConversation.get(conversationId) ?? null;
}

export function writeConversationComposerDraft(conversationId: string, draft: string): void {
  if (draft.length === 0) {
    draftsByConversation.delete(conversationId);
    return;
  }
  draftsByConversation.set(conversationId, draft);
}
