import { ArrowUp, Brain, FolderOpen, Paperclip, Play, Square, X } from 'lucide-react';
import { useCallback, useRef, useState } from 'react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Select, Textarea } from '@shared/components';
import type { ChatMessage } from '@features/chat/data';
import { getSidebarDraggingConversationId } from '@shared/lib/preferences/sidebarPreferences';
import { useChatStreaming } from '@features/chat/hooks/useChatStreaming';
import { useChatStore } from '@features/chat/state';
import { ToolPermissionModeSelector } from '@features/projects/components';
import { modelRouteKey } from '@entities/model/modelPreferences';
import { rateLimitForModel, useOAuthRateLimits } from '@features/provider-settings/data';
import { formatKeyboardShortcut, useKeyboardShortcutsStore } from '@shared/lib/preferences/keyboardShortcutsStore';
import { AttachmentLibraryDialog } from './AttachmentLibraryDialog';
import { ChatSlashCommandMenu } from './ChatSlashCommandMenu';
import { ChatSlashModelMenu } from './ChatSlashModelMenu';
import { ComposerFeedbackList, ComposerModelBanner } from './ComposerAlerts';
import { ComposerContextRow } from './ComposerContextRow';
import { GoalStatusBar } from './GoalStatusBar';
import {
  effectiveContextWindow,
  imageAttachmentExtensions,
  providerNameForModel,
  reasoningLevelLabel,
  textAttachmentExtensions,
} from '@entities/model/composerUtils';
import { ModelPicker } from './ModelPicker';
import { PendingAttachmentGallery } from './PendingAttachmentGallery';
import { UserQuestionPanel } from './UserQuestionPanel';
import { useComposerState } from './useComposerState';

export interface ChatComposerProps {
  onOpenSettings(): void;
  onRequestTemporaryExit(action: () => void | Promise<void>): void;
  onBeforeSend?: (providerId: string, providerName: string) => Promise<boolean>;
  isTemporaryChat?: boolean;
  droppedFiles?: { id: number; files: File[] } | null;
  targetConversationId?: string | null;
  messagesOverride?: readonly ChatMessage[];
  isLoadingMessagesOverride?: boolean;
  questionFocusTarget: { conversationId: string; groupId: string } | null;
  onQuestionFocusHandled(): void;
  fileChanges?: { addedLines: number; removedLines: number; countsUnavailable: boolean; onOpen(): void };
}

export function ChatComposer({
  isTemporaryChat = false,
  droppedFiles,
  onBeforeSend,
  onOpenSettings,
  onRequestTemporaryExit,
  targetConversationId,
  messagesOverride,
  isLoadingMessagesOverride,
  questionFocusTarget,
  onQuestionFocusHandled,
  fileChanges,
}: ChatComposerProps) {
  const { t } = useTranslation();
  const formRef = useRef<HTMLFormElement>(null);
  const sendMessageBinding = useKeyboardShortcutsStore((state) => state.bindings.sendMessage);
  const sendShortcutLabel = sendMessageBinding ? formatKeyboardShortcut(sendMessageBinding) : '';

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
    addExistingAttachments,
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
    canResumeInterruptedRun,
    resumeInterruptedRun,
    ghostTextSuggestion,
  } = useComposerState({
    droppedFiles,
    isTemporaryChat,
    onBeforeSend,
    onRequestTemporaryExit,
    formRef,
    messagesOverride,
    isLoadingMessagesOverride,
    targetConversationId,
  });
  const oauthRateLimits = useOAuthRateLimits();
  const selectedModelRateLimited = selectedModel ? rateLimitForModel(selectedModel, oauthRateLimits) !== null : false;

  const models = useChatStore((state) => state.models);
  const modelProviders = useChatStore((state) => state.modelProviders);
  const conversations = useChatStore((state) => state.conversations);
  const projects = useChatStore((state) => state.projects);
  const projectsStatus = useChatStore((state) => state.projectsStatus);
  const pendingProjectId = useChatStore((state) => state.pendingProjectId);
  const hiddenModelKeys = useChatStore((state) => state.hiddenModelKeys);
  const storeActiveConversationId = useChatStore((state) => state.activeConversationId);
  const temporaryChatId = useChatStore((state) => state.temporaryChat?.id ?? null);
  const activeGoal = useChatStore((state) => state.activeGoal);
  const activeConversationId = targetConversationId === undefined ? storeActiveConversationId : targetConversationId;
  const storeMessages = useChatStore((state) => state.messages);
  const messages = messagesOverride ?? storeMessages;
  const isUpdatingConversationModel = useChatStore((state) => state.isUpdatingConversationModel);
  const selectedReasoningEffort = useChatStore((state) => state.selectedReasoningEffort);
  const modelStatus = useChatStore((state) => state.modelStatus);
  const modelFavorites = useChatStore((state) => state.modelFavorites);
  const chatGptWorkspaceSelectionRequired = useChatStore((state) => state.chatGptWorkspaceSelectionRequired);
  const toggleFavoriteModel = useChatStore((state) => state.toggleFavoriteModel);
  const removeModelFavorite = useChatStore((state) => state.removeModelFavorite);
  const selectConversationModel = useChatStore((state) => state.selectConversationModel);
  const selectModel = useChatStore((state) => state.selectModel);
  const setPendingProjectId = useChatStore((state) => state.setPendingProjectId);
  const selectReasoningEffort = useChatStore((state) => state.selectReasoningEffort);
  const chatErrorCode = useChatStore((state) => state.errorCode);
  const chatErrorMessage = useChatStore((state) => state.errorMessage);
  const opencodeRetryAtUnixMs = useChatStore((state) => state.opencodeRetryAtUnixMs);
  const modelError = useChatStore((state) => state.modelError);
  const modelContextLimits = useChatStore((state) => state.modelContextLimits);
  const { stopResponse } = useChatStreaming();

  const selectableModels = useMemo(
    () => models.filter((model) => !hiddenModelKeys.includes(modelRouteKey(model))),
    [models, hiddenModelKeys],
  );
  const contextWindow = useMemo(
    () => (selectedModel === undefined ? null : effectiveContextWindow(selectedModel, modelContextLimits)),
    [selectedModel, modelContextLimits],
  );
  const activeConversation = useMemo(
    () => conversations.find((conversation) => conversation.id === activeConversationId),
    [conversations, activeConversationId],
  );
  const contextProjectId = activeConversation?.projectId ?? pendingProjectId;
  const contextToolPermissionMode = composerMode === 'plan' ? 'plan' : selectedPermissionMode;
  const isTargetPlaceholder = targetConversationId === null;
  const isAttachmentSelectionDisabled =
    isTargetPlaceholder ||
    pickerModels.length === 0 ||
    isSending ||
    isReadingAttachments ||
    isUpdatingConversationModel;
  const placeholderHint = useMemo(
    () =>
      isTargetPlaceholder
        ? t('conversationUnavailable')
        : !sendMessageBinding
          ? t('composerPlaceholderNoShortcut')
          : t('composerPlaceholderShortcut', { shortcut: sendShortcutLabel }),
    [isTargetPlaceholder, sendMessageBinding, sendShortcutLabel, t],
  );
  const sendTooltip = useMemo(
    () =>
      !sendMessageBinding
        ? t('composerSendTooltipNoShortcut')
        : t('composerSendTooltipShortcut', { shortcut: sendShortcutLabel }),
    [sendMessageBinding, sendShortcutLabel, t],
  );

  const [isComposerDraggingOver, setIsComposerDraggingOver] = useState(false);
  const [isAttachmentLibraryOpen, setIsAttachmentLibraryOpen] = useState(false);
  const composerDragDepthRef = useRef(0);

  // Memoize drag handlers to prevent unnecessary re-renders
  const handleComposerDragEnter = useCallback(
    (event: React.DragEvent) => {
      if (getSidebarDraggingConversationId() !== null) return;
      const types = Array.from(event.dataTransfer?.types ?? []);
      const isConv = types.includes('application/x-openchat-conversation') || types.includes('text/plain');
      const isFiles = types.includes('Files');
      if (!isFiles && !isConv) return;
      event.preventDefault();
      if (isFiles) {
        composerDragDepthRef.current += 1;
        setIsComposerDraggingOver(true);
      }
    },
    [],
  );

  const handleComposerDragOver = useCallback((event: React.DragEvent) => {
    if (getSidebarDraggingConversationId() !== null) return;
    const types = Array.from(event.dataTransfer?.types ?? []);
    const isConv = types.includes('application/x-openchat-conversation') || types.includes('text/plain');
    if (!types.includes('Files') && !isConv) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  }, []);

  const handleComposerDragLeave = useCallback(() => {
    composerDragDepthRef.current -= 1;
    if (composerDragDepthRef.current <= 0) {
      composerDragDepthRef.current = 0;
      setIsComposerDraggingOver(false);
    }
  }, []);

  const handleComposerDrop = useCallback(
    (event: React.DragEvent) => {
      if (getSidebarDraggingConversationId() !== null) return;
      const types = Array.from(event.dataTransfer?.types ?? []);
      const isConv = types.includes('application/x-openchat-conversation') || types.includes('text/plain');
      if (types.includes('Files')) {
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
      if (isConv) {
        event.preventDefault();
        // Allow bubble to workspace
      }
    },
    [addAttachments],
  );

  const handleComposerPaste = useCallback(
    (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
      if (
        isTargetPlaceholder ||
        pickerModels.length === 0 ||
        isSending ||
        isReadingAttachments ||
        isUpdatingConversationModel
      ) {
        return;
      }

      const supportedImageExtensions: Record<string, string> = {
        'image/gif': 'gif',
        'image/jpeg': 'jpg',
        'image/jpg': 'jpg',
        'image/png': 'png',
        'image/webp': 'webp',
      };
      const images = Array.from(event.clipboardData.items)
        .filter((item) => item.kind === 'file' && item.type.startsWith('image/'))
        .map((item) => {
          const file = item.getAsFile();
          if (!file) return null;
          const extension = supportedImageExtensions[file.type.toLowerCase()];
          return extension
            ? new File([file], `pasted-image.${extension}`, { lastModified: file.lastModified, type: file.type })
            : file;
        })
        .filter((file): file is File => file !== null);

      if (images.length === 0) return;
      event.preventDefault();
      void addAttachments(images);
    },
    [addAttachments, isReadingAttachments, isSending, isTargetPlaceholder, isUpdatingConversationModel, pickerModels],
  );

  return (
    <div className="shrink-0 px-6 pb-5 pt-3">
      <div className="composer-container mx-auto max-w-conversation">
        <ComposerModelBanner
          hasModels={models.length > 0}
          hasVisibleModels={selectableModels.length > 0}
          isNewChat={activeConversationId === null}
          onOpenSettings={onOpenSettings}
        />

        <ComposerFeedbackList
          attachmentErrorKey={attachmentErrorKey}
          chatErrorCode={chatErrorCode}
          chatErrorMessage={chatErrorMessage}
          opencodeRetryAtUnixMs={opencodeRetryAtUnixMs}
          commandFeedbackKey={commandFeedbackKey}
          successFeedback={successFeedback}
        />

        <ComposerContextRow
          activeConversationId={activeConversationId}
          contextProjectId={contextProjectId}
          contextToolPermissionMode={contextToolPermissionMode}
          contextWindow={contextWindow}
          fileChanges={fileChanges}
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

        {activeGoal?.conversationId === activeConversationId ? <GoalStatusBar goal={activeGoal} /> : null}

        <UserQuestionPanel
          conversationId={activeConversationId}
          onFocusHandled={onQuestionFocusHandled}
          questionFocusTarget={questionFocusTarget}
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
            disabled={isAttachmentSelectionDisabled}
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
                chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
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
            <Textarea
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
              className="composer-message-input w-full bg-transparent text-composer-input text-text placeholder:text-secondary-text outline-none focus-visible:outline-none focus-visible:ring-0 focus-visible:ring-offset-0 disabled:opacity-50"
              disabled={isTargetPlaceholder || pickerModels.length === 0 || isUpdatingConversationModel}
              id="chat-message"
              appearance="bare"
              fieldLayout="none"
              placeholder={ghostTextSuggestion ?? placeholderHint}
              title={sendTooltip}
              wrap="soft"
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
              onPaste={handleComposerPaste}
              ref={inputRef}
              rows={1}
              value={messageDraft}
            />
          </div>
          <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <Button
                aria-label={t('attachmentChooseFile')}
                disabled={isAttachmentSelectionDisabled}
                onClick={() => filePickerRef.current?.click()}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Paperclip aria-hidden="true" size={18} />
              </Button>
              <Button
                aria-label={t('attachmentChooseFromLibrary')}
                disabled={isAttachmentSelectionDisabled}
                onClick={() => setIsAttachmentLibraryOpen(true)}
                size="icon"
                title={t('attachmentChooseFromLibrary')}
                type="button"
                variant="ghost"
              >
                <FolderOpen aria-hidden="true" size={18} />
              </Button>
              <ModelPicker
                canSelectProvider
                showFastModeControl={!isTemporaryChat}
                fastModeAvailable={fastModeAvailable}
                fastModeEnabled={fastModeEnabled}
                fastModeLoading={fastModePreferenceState === 'loading'}
                fastModeSaving={fastModePreferenceState === 'saving'}
                emptyModelsLabel={t(modelError ? 'modelsUnavailable' : 'noModelsAvailable')}
                chatGptWorkspaceSelectionRequired={chatGptWorkspaceSelectionRequired}
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
                  void selectConversationModel(model.key, targetConversationId ?? undefined).then((updated) => {
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
                onToggleFavorite={(model) => {
                  void toggleFavoriteModel(model);
                }}
                query={modelQuery}
                selectedModel={selectedModel ?? null}
              />
              {!isTemporaryChat && selectedModel?.supportsReasoning ? (
                <div className="w-fit min-w-0 max-w-full">
                  <Select
                    aria-label={t('reasoning')}
                    className="openchat-dropdown-trigger--subtle min-w-0 max-w-full py-2 pl-2.5 pr-2 text-sm"
                    disabled={isTargetPlaceholder || isSending || isUpdatingConversationModel}
                    label={t('reasoning')}
                    leadingIcon={<Brain aria-hidden="true" size={16} />}
                    onChange={(event) => void selectReasoningEffort(event.currentTarget.value || null)}
                    options={[
                      {
                        value: '',
                        label: t('reasoningDefault'),
                        icon: <Brain aria-hidden="true" size={16} />,
                      },
                      ...(selectedModel.reasoningLevels ?? []).map((level) => ({
                        value: level,
                        label: reasoningLevelLabel(level, t),
                        icon: <Brain aria-hidden="true" size={16} />,
                      })),
                    ]}
                    value={selectedReasoningEffort ?? ''}
                    visuallyHiddenLabel
                  />
                </div>
              ) : null}
              <ToolPermissionModeSelector onModeChange={setSelectedPermissionMode} />
            </div>
            {isSending && messageDraft.trim().length === 0 ? (
              <Button
                aria-label={t('stop')}
                className="size-10"
                onClick={() => void stopResponse(activeConversationId ?? temporaryChatId)}
                size="icon"
                variant="secondary"
              >
                <Square aria-hidden="true" fill="currentColor" size={14} />
              </Button>
            ) : canResumeInterruptedRun ? (
              <Button
                aria-label={t('continueLabel')}
                className="size-10"
                disabled={isTargetPlaceholder || isReadingAttachments || isUpdatingConversationModel || isSendBlocked}
                onClick={resumeInterruptedRun}
                size="icon"
                title={t('continueLabel')}
                type="button"
                variant="primary"
              >
                <Play aria-hidden="true" size={18} />
              </Button>
            ) : (
              <Button
                aria-label={isSending ? t('composerSteerResponse') : sendTooltip}
                className="size-10"
                disabled={
                  isTargetPlaceholder ||
                  pickerModels.length === 0 ||
                  !selectedModel?.key ||
                  messageDraft.trim().length === 0 ||
                  isReadingAttachments ||
                  isUpdatingConversationModel ||
                  isSendBlocked ||
                  selectedModelRateLimited
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

        <AttachmentLibraryDialog
          disabled={isAttachmentSelectionDisabled}
          onOpenChange={setIsAttachmentLibraryOpen}
          onSelect={(item) => addExistingAttachments([item])}
          open={isAttachmentLibraryOpen}
        />

      </div>
    </div>
  );
}
