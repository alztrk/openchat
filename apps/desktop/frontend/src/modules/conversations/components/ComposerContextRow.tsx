import { Folder } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Select } from '@/design-system';
import type { PendingChatAttachment } from '@/modules/conversations/data/chatAttachments';
import type { ChatMessage } from '@/modules/conversations/data/chatStorage';
import type { ToolPermissionMode } from '@/modules/projects/data/projectPreferences';
import type { ChatModel } from '@/modules/providers/data/chatModels';
import { ContextUsageIndicator } from './ContextUsageIndicator';

export interface ComposerContextRowProps {
  isTemporaryChat: boolean;
  activeConversationId: string | null;
  isSending: boolean;
  projects: Array<{ id: string; name: string }>;
  projectsStatus: 'idle' | 'loading' | 'ready' | 'error';
  contextProjectId: string | null;
  onSelectProject(projectId: string | null): void;
  selectedModel: ChatModel | undefined;
  contextToolPermissionMode: ToolPermissionMode | 'plan' | null;
  contextWindow: number | null;
  messageDraft: string;
  messages: ChatMessage[];
  pendingAttachments: PendingChatAttachment[];
}

export function ComposerContextRow({
  isTemporaryChat,
  activeConversationId,
  isSending,
  projects,
  projectsStatus,
  contextProjectId,
  onSelectProject,
  selectedModel,
  contextToolPermissionMode,
  contextWindow,
  messageDraft,
  messages,
  pendingAttachments,
}: ComposerContextRowProps) {
  const { t } = useTranslation();

  return (
    <div className="composer-context-row flex min-w-0 items-center justify-between gap-3">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <Folder aria-hidden="true" className="shrink-0 text-secondary-icon" size={16} />
        <Select
          aria-label={t('selectProject')}
          className="min-h-8 max-w-64 border-0 bg-transparent px-2 text-xs hover:bg-hover focus-visible:ring-0"
          disabled={
            isTemporaryChat ||
            activeConversationId !== null ||
            isSending ||
            projectsStatus !== 'ready' ||
            projects.length === 0
          }
          label={t('selectProject')}
          onChange={(event) => onSelectProject(event.target.value || null)}
          options={
            projects.length === 0
              ? [
                  {
                    value: '',
                    label: t(projectsStatus === 'error' ? 'projectLoadFailed' : 'noProjects'),
                    disabled: true,
                  },
                ]
              : [
                  { value: '', label: t('noProjectSelected') },
                  ...projects.map((project) => ({ value: project.id, label: project.name })),
                ]
          }
          value={contextProjectId ?? ''}
          visuallyHiddenLabel
        />
      </div>
      {selectedModel && contextToolPermissionMode !== null ? (
        <ContextUsageIndicator
          contextWindow={contextWindow}
          conversationId={activeConversationId}
          draft={messageDraft}
          isSending={isSending}
          messages={messages}
          model={selectedModel}
          pendingAttachments={pendingAttachments}
          projectId={contextProjectId}
          toolPermissionMode={contextToolPermissionMode}
        />
      ) : selectedModel ? (
        <p className="max-w-48 text-xs text-destructive" role="alert">
          {t('toolPermissionSettingsLoadFailed')}
        </p>
      ) : null}
    </div>
  );
}
