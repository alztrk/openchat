import { useEffect, useRef } from 'react';
import {
  getWorkflowAfterResponseDraft,
  type WorkflowResponseSnapshot,
} from '@features/chat/data/workflowTemplateCore';

interface UseWorkflowAfterResponsePromptInput {
  response: WorkflowResponseSnapshot | null;
  activeConversationId: string | null;
  afterResponsePrompt: string | null;
  draft: string;
  pendingAttachmentCount: number;
  isReadingAttachments: boolean;
  responseNeedsInput: boolean;
  disabled: boolean;
  setDraft(value: string): void;
}

export function useWorkflowAfterResponsePrompt({
  response,
  activeConversationId,
  afterResponsePrompt,
  draft,
  pendingAttachmentCount,
  isReadingAttachments,
  responseNeedsInput,
  disabled,
  setDraft,
}: UseWorkflowAfterResponsePromptInput): void {
  const previousResponseRef = useRef<WorkflowResponseSnapshot | null>(null);

  useEffect(() => {
    const previous = previousResponseRef.current;
    previousResponseRef.current = response;
    const nextDraft = getWorkflowAfterResponseDraft({
      previous,
      current: response,
      activeConversationId,
      afterResponsePrompt,
      draft,
      pendingAttachmentCount,
      isReadingAttachments,
      responseNeedsInput,
      disabled,
    });
    if (nextDraft !== null) setDraft(nextDraft);
  }, [
    activeConversationId,
    afterResponsePrompt,
    disabled,
    draft,
    isReadingAttachments,
    pendingAttachmentCount,
    response,
    responseNeedsInput,
    setDraft,
  ]);
}
