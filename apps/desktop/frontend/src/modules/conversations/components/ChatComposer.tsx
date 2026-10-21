import { ArrowUp, Paperclip, Square, X } from 'lucide-react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/design-system';
import { getSidebarDraggingConversationId } from '@/modules/conversations/data/sidebarPreferences';
import { useChatStreaming } from '@/modules/conversations/hooks/useChatStreaming';
import { useChatStore } from '@/modules/conversations/stores/chatStore';
import { ToolPermissionModeSelector } from '@/modules/projects/components/ToolPermissionModeSelector';
import { modelRouteKey } from '@/modules/providers/data/modelPreferences';
import { ChatSlashCommandMenu } from './ChatSlashCommandMenu';
import { ChatSlashModelMenu } from './ChatSlashModelMenu';
import { ComposerFeedbackList, ComposerModelBanner } from './ComposerAlerts';
import { ComposerContextRow } from './ComposerContextRow';
import {
  effectiveContextWindow,
  imageAttachmentExtensions,
  providerNameForModel,
  textAttachmentExtensions,
} from './composerUtils';
import { ModelPicker } from './ModelPicker';
import { PendingAttachmentGallery } from './PendingAttachmentGallery';
import { useComposerState } from './useComposerState';

export interface ChatComposerProps {
  onOpenSettings(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  onBeforeSend?: (providerId: string, providerName: string) => Promise<boolean>;
  isTemporaryChat?: boolean;
  droppedFiles?: { id: number; files: File[] } | null;
  targetConversationId?: string | null;
}

export function ChatComposer({
  isTemporaryChat = false,
  droppedFiles,
  onBeforeSend,
  onOpenSettings,
  onRequestTemporaryExit,
  targetConversationId,
}: ChatComposerProps) {
  const { t } = useTranslation();
  const formRef = useRef<HTMLFormElement>(null);

  const {
    inputRef,
    filePickerRef,
    messageDraft,
    setMessageDraft,
    composerMode,
    setComposerMode,
    selectedPermissionMode,
    setSelectedPermissionMode,
    pendingAttachments,
    isReadingAttachments,
    attachmentErrorKey,
    commandFeedbackKey,
    setCommandFeedbackKey,
    successFeedback,
    slashCommands,
    slashCommandIndex,
    setSlashCommandIndex,
    selectedSlashCommand,
    slashModelMenuOpen,
    slashModelIndex,
    setSlashModelIndex,
    slashModelCandidates,
    selectedSlashModel,
    modelPickerOpen,
    setModelPickerOpen,
    modelQuery,
    setModelQuery,
    fastModeEnabled,
    fastModeAvailable,
    fastModePreferenceState,
    toggleFastMode,
    addAttachments,
    removeAttachment,
    selectSlashCommand,
    selectSlashModel,
    selectFavoriteModel,
    handleKeyDown,
    send,
    setSlashSuggestionsDismissed,
    pickerModels,
    selectedModel,
    isSendBlocked,
    isSending,
    submissionMode,
    ghostTextSuggestion,
  } = useComposerState({
    droppedFiles,
    isTemporaryChat,
    onBeforeSend,
    onRequestTemporaryExit,
    formRef,
    targetConversationId,
  });

  const models = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);
  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const pendingProjectId = useChatStore((state) => state.pendingProjectId);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const selectedModelKey = useChatStore((state) => state.selectedModelKey);
  const activeConversationId = useChatStore((state) => state.activeConversationId);
  const messages = useChatStore((state) => state.messages);
  const isUpdatingConversationModel = useChatStore((state) => state.isUpdatingConversationModel);
  const selectedReasoningEffort = useChatStore((state) => state.selectedReasoningEffort);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);
  const selectConversationModel = useChatStore((state) => state.selectConversationModel);
  const selectModel = useChatStore((state) => state.selectModel);
  const setPendingProjectId = useChatStore((state) => state.setPendingProjectId);
  const selectReasoningEffort = useChatStore((state) => state.selectReasoningEffort);
  const chatErrorCode = useChatStore((state) => state.errorCode);
  const modelError = useChatStore((state) => state.modelError);
  const modelContextLimits = useChatStore((state) => state.modelContextLimits);
  const { stopResponse } = useChatStreaming();

  const selectableModels = models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model)));
  const contextWindow = selectedModel === undefined ? null : effectiveContextWindow(selectedModel, modelContextLimits);
  const activeConversation = conversations.find((conversation) => conversation.id === activeConversationId);
  const contextProjectId = activeConversation?.projectId ?? pendingProjectId;
  const contextToolPermissionMode = composerMode === 'plan' ? 'plan' : selectedPermissionMode;
  const isTargetPlaceholder = targetConversationId === null;
  const placeholderHint = isTargetPlaceholder
    ? t('splitViewEmptyHint')
    : submissionMode === 'ctrlEnter'
      ? t('composerPlaceholderCtrlEnter')
      : t('composerPlaceholderEnter');
  const sendTooltip =
    submissionMode === 'ctrlEnter' ? t('composerSendTooltipCtrlEnter') : t('composerSendTooltipEnter');

  const [isComposerDraggingOver, setIsComposerDraggingOver] = useState(false);
  const composerDragDepthRef = useRef(0);

  const isComposerConversationDrag = (event: React.DragEvent) => {
    if (getSidebarDraggingConversationId() !== null) return true;
    const types = Array.from(event.dataTransfer?.types ?? []);
    return types.includes('application/x-openchat-conversation') || types.includes('text/plain');
  };

  const handleComposerDragEnter = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    const isConv = isComposerConversationDrag(event);
    if (!isFiles && !isConv) return;
    event.preventDefault();
    if (isFiles) {
      composerDragDepthRef.current += 1;
      setIsComposerDraggingOver(true);
    }
  };

  const handleComposerDragOver = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    const isConv = isComposerConversationDrag(event);
    if (!isFiles && !isConv) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };

  const handleComposerDragLeave = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    if (!isFiles) return;
    event.preventDefault();
    composerDragDepthRef.current -= 1;
    if (composerDragDepthRef.current <= 0) {
      composerDragDepthRef.current = 0;
      setIsComposerDraggingOver(false);
    }
  };

  const handleComposerDrop = (event: React.DragEvent) => {
    const isFiles = Array.from(event.dataTransfer?.types ?? []).includes('Files');
    if (isFiles) {
      event.preventDefault();
      event.stopPropagation();
      composerDragDepthRef.current = 0;
      setIsComposerDraggingOver(false);
      const files = Array.from(event.dataTransfer.files ?? []);
      if (files.length > 0) {
        void addAttachments(files);
      }
      return;
    }
    if (isComposerConversationDrag(event)) {
      event.preventDefault();
      // Allow bubble to workspace
    }
  };

  return (
    <div className="shrink-0 px-4 pb-5 pt-3 sm:px-8">
      <div className="composer-container mx-auto max-w-composer">
        <ComposerModelBanner
          hasModels={models.length > 0}
          hasVisibleModels={selectableModels.length > 0}
          isNewChat={activeConversationId === null}
          onOpenSettings={onOpenSettings}
        />

        <ComposerContextRow
          activeConversationId={activeConversationId}
          contextProjectId={contextProjectId}
          contextToolPermissionMode={contextToolPermissionMode}
          contextWindow={contextWindow}
          isSending={isSending}
          isTemporaryChat={isTemporaryChat}
          messageDraft={messageDraft}
          messages={messages}
          onSelectProject={setPendingProjectId}
          pendingAttachments={pendingAttachments}
          projects={projects}
          projectsStatus={projectsStatus}
          selectedModel={selectedModel}
        />

        <form
          className={`composer-surface rounded-composer transition-colors ${
            isComposerDraggingOver ? 'border-accent bg-accent/5 ring-2 ring-accent' : ''
          }`}
          onDragEnter={handleComposerDragEnter}
          onDragLeave={handleComposerDragLeave}
          onDragOver={handleComposerDragOver}
          onDrop={handleComposerDrop}
          onSubmit={send}
          ref={formRef}
        >
          <input
            accept={
              selectedModel?.supportsImages === true
                ? `${imageAttachmentExtensions},${textAttachmentExtensions}`
                : textAttachmentExtensions
            }
            aria-label={t('attachmentChooseFile')}
            className="sr-only"
            disabled={pickerModels.length === 0 || isSending || isReadingAttachments || isUpdatingConversationModel}
            multiple
            onChange={(event) => {
              const files = Array.from(event.currentTarget.files ?? []);
              event.currentTarget.value = '';
              if (files.length > 0) void addAttachments(files);
            }}
            ref={filePickerRef}
            type="file"
          />
          {pendingAttachments.length > 0 ? (
            <PendingAttachmentGallery
              attachments={pendingAttachments}
              disabled={isSending || isReadingAttachments}
              onRemove={removeAttachment}
            />
          ) : null}
          {!isTemporaryChat && composerMode !== 'normal' ? (
            <div className="mb-3 flex items-center gap-2">
              <Button
                aria-label={t(composerMode === 'plan' ? 'toolPermissionPlanOff' : 'composerGoalModeOff')}
                className="min-h-9"
                onClick={() => setComposerMode('normal')}
                size="small"
                variant="secondary"
              >
                <span>{t(composerMode === 'plan' ? 'toolPermissionPlan' : 'composerGoalModeLabel')}</span>
                <X aria-hidden="true" size={14} />
              </Button>
            </div>
          ) : null}
          <div className="relative">
            {slashModelMenuOpen ? (
              <ChatSlashModelMenu
                canSelect={!isSending && !isUpdatingConversationModel}
                isLoading={modelStatus === 'loading'}
                models={slashModelCandidates}
                onSelect={(model) => void selectSlashModel(model)}
                providerLabel={(model) => providerNameForModel(model, t)}
                selectedIndex={slashModelIndex}
              />
            ) : slashCommands.length > 0 ? (
              <ChatSlashCommandMenu
                commands={slashCommands}
                onSelect={selectSlashCommand}
                selectedIndex={slashCommandIndex}
              />
            ) : null}
            {messageDraft.length === 0 ? (
              ghostTextSuggestion ? (
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute left-0 top-0 flex items-center gap-1.5 text-composer-input text-secondary-text/80 select-none"
                >
                  <kbd className="rounded border border-border bg-surface px-1.5 py-0.5 text-2xs font-semibold text-text shadow-sm">
                    Tab
                  </kbd>
                  <span className="italic">{ghostTextSuggestion}</span>
                </span>
              ) : (
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute left-0 top-0 text-composer-input text-secondary-text"
                >
                  {placeholderHint}
                </span>
              )
            ) : null}
            <textarea
              aria-activedescendant={
                slashModelMenuOpen
                  ? selectedSlashModel
                    ? `chat-slash-model-${encodeURIComponent(selectedSlashModel.key)}`
                    : undefined
                  : selectedSlashCommand
                    ? `chat-slash-command-${selectedSlashCommand}`
                    : undefined
              }
              aria-autocomplete={slashModelMenuOpen || slashCommands.length > 0 ? 'list' : 'none'}
              aria-controls={
                slashModelMenuOpen
                  ? 'chat-slash-model-list'
                  : slashCommands.length > 0
                    ? 'chat-slash-command-list'
                    : undefined
              }
              role="combobox"
              aria-expanded={slashModelMenuOpen || slashCommands.length > 0}
              aria-label={placeholderHint}
              className="composer-message-input w-full bg-transparent text-composer-input text-text outline-none focus-visible:outline-none focus-visible:ring-0 focus-visible:ring-offset-0 disabled:opacity-50"
              disabled={isTargetPlaceholder || pickerModels.length === 0 || isUpdatingConversationModel}
              id="chat-message"
              onChange={(event) => {
                const input = event.currentTarget;
                setMessageDraft(input.value);
                setSlashSuggestionsDismissed(slashModelMenuOpen);
                setSlashCommandIndex(0);
                if (slashModelMenuOpen) setSlashModelIndex(0);
                setCommandFeedbackKey(null);
                input.style.height = 'auto';
                input.style.height = `${input.scrollHeight}px`;
              }}
              onKeyDown={handleKeyDown}
              ref={inputRef}
              rows={1}
              value={messageDraft}
            />
          </div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <Button
                aria-label={t('attachmentChooseFile')}
                disabled={isTargetPlaceholder || pickerModels.length === 0 || isSending || isReadingAttachments}
                onClick={() => filePickerRef.current?.click()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Paperclip aria-hidden="true" size={18} />
              </Button>
              <ModelPicker
                canSelectProvider={activeConversationId === null}
                showFastModeControl={!isTemporaryChat}
                fastModeAvailable={fastModeAvailable}
                fastModeEnabled={fastModeEnabled}
                fastModeLoading={fastModePreferenceState === 'loading'}
                fastModeSaving={fastModePreferenceState === 'saving'}
                showReasoningSelector={!isTemporaryChat && (selectedModel?.supportsReasoning ?? false)}
                reasoningDisabled={isSending || isUpdatingConversationModel}
                reasoningEffort={selectedReasoningEffort}
                reasoningLevels={selectedModel?.reasoningLevels ?? []}
                emptyModelsLabel={t(modelError ? 'modelsUnavailable' : 'noModelsAvailable')}
                hiddenModelKeys={hiddenModelKeys}
                isOpen={modelPickerOpen}
                knownModels={models}
                modelFavorites={modelFavorites}
                modelProviders={modelProviders}
                models={pickerModels}
                disabled={isSending || isUpdatingConversationModel}
                onOpenChange={(open) => {
                  setModelPickerOpen(open);
                }}
                onQueryChange={setModelQuery}
                onSelect={(model) => {
                  setModelPickerOpen(false);
                  setCommandFeedbackKey(null);
                  void selectConversationModel(model.key).then((updated) => {
                    if (!updated) setCommandFeedbackKey('slashCommandModelUnavailable');
                    else setCommandFeedbackKey(null);
                  });
                }}
                onRemoveFavorite={(favorite) => {
                  void removeModelFavorite(favorite);
                }}
                onSelectFavorite={selectFavoriteModel}
                onSelectProvider={() => {
                  if (activeConversationId === null) selectModel(null);
                  setCommandFeedbackKey(null);
                  setModelQuery('');
                }}
                onFastModeToggle={toggleFastMode}
                onSelectReasoningEffort={(value) => void selectReasoningEffort(value)}
                onToggleFavorite={(model) => {
                  void toggleFavoriteModel(model);
                }}
                query={modelQuery}
                selectedModel={selectedModel ?? null}
              />
              <ToolPermissionModeSelector onModeChange={setSelectedPermissionMode} />
            </div>
            {isSending && messageDraft.trim().length === 0 ? (
              <Button
                aria-label={t('stop')}
                className="size-10"
                onClick={() => void stopResponse()}
                size="icon"
                variant="secondary"
              >
                <Square aria-hidden="true" fill="currentColor" size={14} />
              </Button>
            ) : (
              <Button
                aria-label={isSending ? t('composerSteerResponse') : sendTooltip}
                className="size-10"
                disabled={
                  isTargetPlaceholder ||
                  pickerModels.length === 0 ||
                  !selectedModelKey ||
                  messageDraft.trim().length === 0 ||
                  isReadingAttachments ||
                  isUpdatingConversationModel ||
                  isSendBlocked
                }
                size="icon"
                title={isSending ? t('composerSteerResponse') : sendTooltip}
                type="submit"
                variant="primary"
              >
                <ArrowUp aria-hidden="true" size={20} />
              </Button>
            )}
          </div>
        </form>

        <ComposerFeedbackList
          attachmentErrorKey={attachmentErrorKey}
          chatErrorCode={chatErrorCode}
          commandFeedbackKey={commandFeedbackKey}
          modelError={modelError}
          successFeedback={successFeedback}
        />
      </div>
    </div>
  );
}
