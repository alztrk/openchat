let pendingComposerDraft: string | null = null;

export function setPendingComposerDraft(draft: string): void {
  pendingComposerDraft = draft;
}

export function readPendingComposerDraft(): string | null {
  return pendingComposerDraft;
}

export function clearPendingComposerDraft(draft: string): void {
  if (pendingComposerDraft === draft) pendingComposerDraft = null;
}
