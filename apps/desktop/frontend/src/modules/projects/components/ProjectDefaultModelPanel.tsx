import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/design-system';
import type { ChatProject } from '@/modules/conversations/data/chatStorage';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { writeProjectDefaultModelPreference } from '@/modules/projects/data/projectPreferences';
import { defaultPreferenceForModel } from '@/modules/providers/data/modelPreferences';

interface ProjectDefaultModelPanelProps {
  project: ChatProject;
  onClose(): void;
}

export function ProjectDefaultModelPanel({ project, onClose }: ProjectDefaultModelPanelProps) {
  const { t } = useTranslation();
  const [error, setError] = useState(false);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const models = useChatStore((state) => state.models);
  const selectedModel = models.find((model) => model.key === selectedModelKey);

  const saveCurrent = () => {
    if (!selectedModel) return;
    try {
      writeProjectDefaultModelPreference(project.id, defaultPreferenceForModel(selectedModel));
      const state = useChatStore.getState();
      if (state.activeConversationId === null && state.pendingProjectId === project.id) {
        state.startNewChat(project.id);
      }
      setError(false);
      setSavedMessage(t('projectDefaultModelSaved', { model: selectedModel.displayName }));
    } catch {
      setError(true);
      setSavedMessage(null);
    }
  };

  const clear = () => {
    try {
      writeProjectDefaultModelPreference(project.id, null);
      const state = useChatStore.getState();
      if (state.activeConversationId === null && state.pendingProjectId === project.id) {
        state.startNewChat(project.id);
      }
      setError(false);
      setSavedMessage(t('projectDefaultModelCleared'));
    } catch {
      setError(true);
      setSavedMessage(null);
    }
  };

  return (
    <div className="grid gap-4">
      <p className="text-sm text-secondary-text">
        {selectedModel ? selectedModel.displayName : t('projectDefaultModelSelectFirst')}
      </p>
      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {t('projectDefaultModelSaveFailed')}
        </p>
      ) : null}
      {savedMessage ? (
        <p className="text-sm text-success" role="status">
          {savedMessage}
        </p>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button onClick={onClose}>{t('cancel')}</Button>
        <Button onClick={clear} variant="secondary">
          {t('projectDefaultModelClear')}
        </Button>
        <Button disabled={!selectedModel} onClick={saveCurrent} variant="primary">
          {t('projectDefaultModelUseCurrent')}
        </Button>
      </div>
    </div>
  );
}
