import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/data/chat_file_changes_repository.dart';
import 'package:openchat/features/chat/domain/agent_question.dart';
import 'package:openchat/features/chat/domain/agent_goal.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/assistant_message.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_file_changes_panel.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/features/chat/presentation/widgets/file_changes_summary_card.dart';
import 'package:openchat/features/chat/presentation/widgets/goal_status_bar.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_permission_card.dart';
import 'package:openchat/features/chat/presentation/widgets/user_question_card.dart';

class ConversationPane extends StatelessWidget {
  const ConversationPane({
    required this.messageController,
    required this.showHistoryButton,
    required this.onOpenHistory,
    this.historyButtonTooltip,
    required this.onSendMessage,
    this.onRetryResponse,
    this.onBranchMessage,
    this.onStopMessage,
    this.canSendMessage = false,
    this.isSending = false,
    this.isLoadingModels = false,
    this.hasAvailableModels = false,
    this.hasSelectedModel = false,
    this.modelCatalogFailed = false,
    this.onOpenConnections,
    this.onRetryModels,
    this.models = const <ChatGptModel>[],
    this.favoriteModels = const <FavoriteModel>[],
    required this.providerId,
    this.isChatGptConnected = false,
    this.chatGptFastModeEnabled = false,
    this.chatGptFastModeAvailable = false,
    this.chatGptFastModeLoading = false,
    this.chatGptFastModeSaving = false,
    this.onChatGptFastModeChanged,
    this.availableProviderIds = const <String>{},
    this.onProviderSelected,
    this.hiddenModelKeys = const <String>{},
    this.selectedModelId,
    this.selectedModelRouteKey,
    this.onModelSelected,
    this.onModelFavoriteChanged,
    this.onFavoriteModelSelected,
    this.reasoningOptions = const <String>[],
    this.showReasoningSelector = false,
    this.onReasoningSelected,
    this.messages = const <ChatMessage>[],
    this.savedOutputMessageIds = const <String>{},
    this.pendingSavedOutputMessageIds = const <String>{},
    this.onToggleSavedOutput,
    this.messagesLoading = false,
    this.showAssistantLoading = false,
    this.messagesErrorDescription,
    this.onRetryMessageHistory,
    this.conversationTitle,
    this.productWorkspaceName,
    this.conversationId,
    this.contextProviderId,
    this.contextModelId,
    this.contextSupportsTools,
    this.contextWindow,
    this.contextConnectionId,
    this.contextWorkspaceId,
    this.conversationMemoryRepository,
    this.fileChangesRepository,
    this.conversationFileChanges = const <ChatFileChange>[],
    this.fileChangesRevision = 0,
    this.isFileChangesPanelOpen = false,
    this.onOpenFileChanges,
    this.onCloseFileChanges,
    this.onFileChangesUpdated,
    this.settingsPreferences,
    this.pendingAttachments = const <ChatAttachment>[],
    this.onAddAttachments,
    this.onRemoveAttachment,
    this.attachmentsEnabled = false,
    this.titleEditRequestId,
    this.onRenameConversation,
    this.onConversationTitleEditFinished,
    this.selectedModelLabel,
    this.modelsEmptyLabel,
    this.assistantModelLabel,
    this.reasoningLevel,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    this.toolPermissionRequest,
    this.isRespondingToToolPermission = false,
    this.toolPermissionError,
    this.onApproveToolPermission,
    this.onDenyToolPermission,
    this.pendingQuestionGroups = const <AgentQuestionGroup>[],
    this.focusedQuestionGroupId,
    this.isResumingQuestion = false,
    this.pendingQuestionError,
    this.onRetryPendingQuestions,
    this.onSubmitQuestionAnswers,
    this.activeGoal,
    this.isChangingGoal = false,
    this.onPauseOrResumeGoal,
    this.onStopGoal,
    this.messageScrollController,
    this.historySearchTargetMessageId,
    this.historySearchTargetRequestId = 0,
    this.onHistorySearchTargetHandled,
    super.key,
  });

  final TextEditingController messageController;
  final ScrollController? messageScrollController;
  final String? historySearchTargetMessageId;
  final int historySearchTargetRequestId;
  final void Function(String messageId, bool found)?
  onHistorySearchTargetHandled;
  final bool showHistoryButton;
  final VoidCallback onOpenHistory;
  final String? historyButtonTooltip;
  final VoidCallback onSendMessage;
  final ValueChanged<ChatMessage>? onRetryResponse;
  final Future<void> Function(ChatMessage message, String editedContent)?
  onBranchMessage;
  final VoidCallback? onStopMessage;
  final bool canSendMessage;
  final bool isSending;
  final bool isLoadingModels;
  final bool hasAvailableModels;
  final bool hasSelectedModel;
  final bool modelCatalogFailed;
  final VoidCallback? onOpenConnections;
  final VoidCallback? onRetryModels;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final bool isChatGptConnected;
  final bool chatGptFastModeEnabled;
  final bool chatGptFastModeAvailable;
  final bool chatGptFastModeLoading;
  final bool chatGptFastModeSaving;
  final ValueChanged<bool>? onChatGptFastModeChanged;
  final Set<String> availableProviderIds;
  final ValueChanged<String>? onProviderSelected;
  final Set<String> hiddenModelKeys;
  final String? selectedModelId;
  final String? selectedModelRouteKey;
  final ValueChanged<ChatGptModel>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    String? sourceConnectionId,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final bool showReasoningSelector;
  final ValueChanged<String?>? onReasoningSelected;
  final List<ChatMessage> messages;
  final Set<String> savedOutputMessageIds;
  final Set<String> pendingSavedOutputMessageIds;
  final ValueChanged<ChatMessage>? onToggleSavedOutput;
  final bool messagesLoading;
  final bool showAssistantLoading;
  final String? messagesErrorDescription;
  final VoidCallback? onRetryMessageHistory;
  final String? conversationTitle;
  final String? productWorkspaceName;
  final String? conversationId;
  final String? contextProviderId;
  final String? contextModelId;
  final bool? contextSupportsTools;
  final int? contextWindow;
  final String? contextConnectionId;
  final String? contextWorkspaceId;
  final ConversationMemoryRepository? conversationMemoryRepository;
  final ChatFileChangesRepository? fileChangesRepository;
  final List<ChatFileChange> conversationFileChanges;
  final int fileChangesRevision;
  final bool isFileChangesPanelOpen;
  final VoidCallback? onOpenFileChanges;
  final VoidCallback? onCloseFileChanges;
  final ValueChanged<List<ChatFileChange>>? onFileChangesUpdated;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final ValueChanged<String>? onRemoveAttachment;
  final bool attachmentsEnabled;
  final String? titleEditRequestId;
  final Future<void> Function(String conversationId, String title)?
  onRenameConversation;
  final VoidCallback? onConversationTitleEditFinished;
  final String? selectedModelLabel;
  final String? modelsEmptyLabel;
  final String? assistantModelLabel;
  final String? reasoningLevel;
  final ToolPermissionMode toolPermissionMode;
  final ValueChanged<ToolPermissionMode>? onToolPermissionModeChanged;
  final ToolPermissionRequest? toolPermissionRequest;
  final bool isRespondingToToolPermission;
  final String? toolPermissionError;
  final VoidCallback? onApproveToolPermission;
  final VoidCallback? onDenyToolPermission;
  final List<AgentQuestionGroup> pendingQuestionGroups;
  final String? focusedQuestionGroupId;
  final bool isResumingQuestion;
  final String? pendingQuestionError;
  final VoidCallback? onRetryPendingQuestions;
  final Future<String?> Function(
    AgentQuestionGroup group,
    List<AgentQuestionAnswer> answers,
  )?
  onSubmitQuestionAnswers;
  final AgentGoal? activeGoal;
  final bool isChangingGoal;
  final VoidCallback? onPauseOrResumeGoal;
  final VoidCallback? onStopGoal;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;

    final content = LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 640;
        final keyboardVisible = MediaQuery.viewInsetsOf(context).bottom > 0;
        final horizontalPadding = compact
            ? OpenChatSpacing.compactPageHorizontal
            : OpenChatSpacing.pageHorizontal;
        final conversationStyle = OpenChatConversationStyle.of(context);
        final composerMaxWidth =
            OpenChatSpacing.composerMaxWidth *
            (conversationStyle.maxWidth / OpenChatSpacing.conversationMaxWidth);
        final latestFileChanges = _mergeConversationFileChanges(
          messages,
          conversationFileChanges,
        );
        final isEmptyConversation =
            messagesErrorDescription == null &&
            !messagesLoading &&
            messages.isEmpty &&
            !showAssistantLoading &&
            historySearchTargetMessageId == null;
        final centerNewConversation =
            isEmptyConversation && !compact && constraints.maxHeight >= 720;
        final header = _ConversationHeader(
          showHistoryButton: showHistoryButton,
          title: conversationTitle ?? l10n.conversationTitle,
          productWorkspaceName: productWorkspaceName,
          conversationId: conversationId,
          titleEditRequestId: titleEditRequestId,
          onRenameConversation: onRenameConversation,
          onTitleEditFinished: onConversationTitleEditFinished,
          onOpenHistory: onOpenHistory,
          historyButtonTooltip: historyButtonTooltip,
        );
        final history = messagesErrorDescription != null
            ? _ConversationMessageError(
                description: messagesErrorDescription!,
                onRetry: onRetryMessageHistory,
              )
            : messagesLoading
            ? Center(
                child: Semantics(
                  label: l10n.messageHistoryLoading,
                  child: SizedBox.square(
                    dimension: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ),
              )
            : isEmptyConversation
            ? _NewConversationEmptyState(
                isLoading: isLoadingModels,
                title: isLoadingModels
                    ? l10n.modelsLoading
                    : !hasAvailableModels
                    ? modelCatalogFailed
                          ? l10n.modelsUnavailable
                          : l10n.noModelConnected
                    : !hasSelectedModel
                    ? l10n.emptyChatSelectModelTitle
                    : l10n.emptyChatWelcomeTitle,
                description: isLoadingModels
                    ? null
                    : !hasAvailableModels
                    ? modelCatalogFailed
                          ? l10n.modelCatalogUnavailable
                          : l10n.noModelConnectedBody
                    : !hasSelectedModel
                    ? l10n.emptyChatSelectModelBody
                    : l10n.emptyChatWelcomeBody,
                onOpenConnections: !isLoadingModels && !hasAvailableModels
                    ? onOpenConnections
                    : null,
                onRetryModels: modelCatalogFailed ? onRetryModels : null,
              )
            : _ConversationHistory(
                messages: messages,
                messagesLoaded: true,
                showAssistantLoading: showAssistantLoading,
                assistantModelLabel: assistantModelLabel,
                onRetryResponse: onRetryResponse,
                onBranchMessage: onBranchMessage,
                onOpenFileChanges: onOpenFileChanges,
                latestFileChanges: latestFileChanges,
                controller: messageScrollController,
                providerId: providerId,
                savedOutputMessageIds: savedOutputMessageIds,
                pendingSavedOutputMessageIds: pendingSavedOutputMessageIds,
                onToggleSavedOutput: onToggleSavedOutput,
                targetMessageId: historySearchTargetMessageId,
                targetRequestId: historySearchTargetRequestId,
                onTargetHandled: onHistorySearchTargetHandled,
              );
        final composerArea = Padding(
          padding: EdgeInsets.fromLTRB(
            horizontalPadding,
            0,
            horizontalPadding,
            keyboardVisible
                ? 0
                : constraints.maxHeight <= 320
                ? OpenChatSpacing.mainSurfaceInset
                : OpenChatSpacing.composerBottomInset,
          ),
          child: Center(
            child: ConstrainedBox(
              constraints: BoxConstraints(maxWidth: composerMaxWidth),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (latestFileChanges.isNotEmpty &&
                      conversationId != null &&
                      fileChangesRepository != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 10),
                      child: Center(
                        child: _FileChangesButton(
                          changes: latestFileChanges,
                          onPressed: onOpenFileChanges,
                        ),
                      ),
                    ),
                  if (pendingQuestionError case final error?) ...[
                    ChatSurfaceCard(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 10,
                      ),
                      child: Row(
                        children: [
                          Icon(
                            LucideIcons.circleAlert,
                            size: 16,
                            color: Theme.of(context).colorScheme.error,
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Semantics(
                              liveRegion: true,
                              child: Text(
                                error,
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ),
                          ),
                          TextButton(
                            onPressed: onRetryPendingQuestions,
                            child: Text(l10n.retry),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(height: 10),
                  ],
                  for (final group in pendingQuestionGroups) ...[
                    UserQuestionCard(
                      group: group,
                      focusOnBuild: group.id == focusedQuestionGroupId,
                      isResuming: isResumingQuestion,
                      onSubmit: (answers) =>
                          onSubmitQuestionAnswers?.call(group, answers) ??
                          Future<String?>.value(l10n.userQuestionUnavailable),
                    ),
                    const SizedBox(height: 10),
                  ],
                  if (toolPermissionRequest case final request?) ...[
                    ToolPermissionCard(
                      request: request,
                      isResponding: isRespondingToToolPermission,
                      errorMessage: toolPermissionError,
                      onApprove: onApproveToolPermission,
                      onDeny: onDenyToolPermission,
                    ),
                    const SizedBox(height: 10),
                  ],
                  if (activeGoal case final goal?) ...[
                    GoalStatusBar(
                      goal: goal,
                      isBusy: isChangingGoal,
                      onPauseOrResume: onPauseOrResumeGoal ?? () {},
                      onStop: onStopGoal ?? () {},
                    ),
                    const SizedBox(height: 10),
                  ],
                  ChatComposer(
                    controller: messageController,
                    onSendMessage: onSendMessage,
                    canSendMessage: canSendMessage,
                    isLoadingModels: isLoadingModels,
                    isSending: isSending,
                    onStopMessage: onStopMessage,
                    modelLabel: selectedModelLabel,
                    models: models,
                    favoriteModels: favoriteModels,
                    providerId: providerId,
                    isChatGptConnected: isChatGptConnected,
                    chatGptFastModeEnabled: chatGptFastModeEnabled,
                    chatGptFastModeAvailable: chatGptFastModeAvailable,
                    chatGptFastModeLoading: chatGptFastModeLoading,
                    chatGptFastModeSaving: chatGptFastModeSaving,
                    onChatGptFastModeChanged: onChatGptFastModeChanged,
                    availableProviderIds: availableProviderIds,
                    onProviderSelected: onProviderSelected,
                    modelsEmptyLabel: modelsEmptyLabel,
                    hiddenModelKeys: hiddenModelKeys,
                    selectedModelId: selectedModelId,
                    selectedModelRouteKey: selectedModelRouteKey,
                    onModelSelected: onModelSelected,
                    onModelFavoriteChanged: onModelFavoriteChanged,
                    onFavoriteModelSelected: onFavoriteModelSelected,
                    reasoningLevel: reasoningLevel,
                    reasoningOptions: reasoningOptions,
                    onReasoningSelected: onReasoningSelected,
                    showReasoningSelector: showReasoningSelector,
                    toolPermissionMode: toolPermissionMode,
                    onToolPermissionModeChanged: onToolPermissionModeChanged,
                    messages: messages,
                    conversationId: conversationId,
                    contextProviderId: contextProviderId,
                    contextModelId: contextModelId,
                    contextSupportsTools: contextSupportsTools,
                    contextWindow: contextWindow,
                    contextConnectionId: contextConnectionId,
                    contextWorkspaceId: contextWorkspaceId,
                    conversationMemoryRepository: conversationMemoryRepository,
                    settingsPreferences: settingsPreferences,
                    pendingAttachments: pendingAttachments,
                    onAddAttachments: onAddAttachments,
                    onRemoveAttachment: onRemoveAttachment,
                    attachmentsEnabled: attachmentsEnabled,
                  ),
                ],
              ),
            ),
          ),
        );
        final showSidePanel =
            isFileChangesPanelOpen &&
            conversationId != null &&
            fileChangesRepository != null;
        final fileChangesPanel = showSidePanel
            ? ChatFileChangesPanel(
                key: ValueKey<String>('file-changes-$conversationId'),
                repository: fileChangesRepository!,
                conversationId: conversationId!,
                refreshRevision: fileChangesRevision,
                onClose: onCloseFileChanges ?? () {},
                onChangesUpdated: onFileChangesUpdated ?? (_) {},
              )
            : null;
        final conversationBody = showSidePanel && constraints.maxWidth < 1120
            ? fileChangesPanel!
            : isEmptyConversation && !centerNewConversation
            ? Center(child: history)
            : history;
        final mainContent = Column(
          children: [
            header,
            if (centerNewConversation)
              Expanded(
                child: LayoutBuilder(
                  builder: (context, bodyConstraints) => SingleChildScrollView(
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        minHeight: bodyConstraints.maxHeight,
                      ),
                      child: Center(
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            history,
                            const SizedBox(height: OpenChatSpacing.lg),
                            composerArea,
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              )
            else ...[
              Expanded(child: conversationBody),
              composerArea,
            ],
          ],
        );

        if (fileChangesPanel == null || constraints.maxWidth < 1120) {
          return mainContent;
        }
        return Row(
          children: [
            Expanded(child: mainContent),
            SizedBox(
              width: math.min(440.0, constraints.maxWidth * 0.38),
              child: fileChangesPanel,
            ),
          ],
        );
      },
    );
    return ColoredBox(
      color: OpenChatPalette.of(context).surface,
      child: SafeArea(left: false, right: false, child: content),
    );
  }
}

class _ConversationMessageError extends StatelessWidget {
  const _ConversationMessageError({required this.description, this.onRetry});

  final String description;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 440),
          child: Semantics(
            liveRegion: true,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  LucideIcons.circleAlert,
                  color: Theme.of(context).colorScheme.error,
                  size: 24,
                ),
                const SizedBox(height: 12),
                Text(
                  description,
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: OpenChatPalette.of(context).secondaryText,
                  ),
                ),
                if (onRetry != null) ...[
                  const SizedBox(height: 16),
                  OutlinedButton.icon(
                    onPressed: onRetry,
                    icon: const Icon(LucideIcons.refreshCw),
                    label: Text(context.openchatL10n.retry),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ConversationHistory extends StatefulWidget {
  const _ConversationHistory({
    required this.messages,
    required this.messagesLoaded,
    required this.showAssistantLoading,
    required this.assistantModelLabel,
    required this.onRetryResponse,
    required this.onBranchMessage,
    required this.controller,
    required this.providerId,
    required this.savedOutputMessageIds,
    required this.pendingSavedOutputMessageIds,
    required this.onToggleSavedOutput,
    required this.onOpenFileChanges,
    required this.latestFileChanges,
    required this.targetMessageId,
    required this.targetRequestId,
    required this.onTargetHandled,
  });

  final List<ChatMessage> messages;
  final bool messagesLoaded;
  final bool showAssistantLoading;
  final String? assistantModelLabel;
  final ValueChanged<ChatMessage>? onRetryResponse;
  final Future<void> Function(ChatMessage message, String editedContent)?
  onBranchMessage;
  final ScrollController? controller;
  final String providerId;
  final Set<String> savedOutputMessageIds;
  final Set<String> pendingSavedOutputMessageIds;
  final ValueChanged<ChatMessage>? onToggleSavedOutput;
  final VoidCallback? onOpenFileChanges;
  final List<ChatFileChange> latestFileChanges;
  final String? targetMessageId;
  final int targetRequestId;
  final void Function(String messageId, bool found)? onTargetHandled;

  @override
  State<_ConversationHistory> createState() => _ConversationHistoryState();
}

class _ConversationHistoryState extends State<_ConversationHistory> {
  static const double _latestMessageThreshold = 96;
  static const double _autoScrollDeadZone = 10;
  static const double _autoScrollSpeedFactor = 5;
  static const double _maximumAutoScrollSpeed = 1400;

  ScrollController? _historyController;
  Timer? _middleScrollTimer;
  Stopwatch? _middleScrollClock;
  int? _middleScrollPointer;
  double? _middleScrollAnchorY;
  double? _middleScrollPointerY;
  bool _followLatestMessage = true;
  bool _isMiddleScrolling = false;
  final GlobalKey _historySliverKey = GlobalKey();
  Timer? _targetHighlightTimer;
  String? _focusedMessageId;
  int? _handledTargetRequestId;
  int _targetAttemptGeneration = 0;
  final Map<String, String> _selectedResponseVersions = <String, String>{};

  @override
  void initState() {
    super.initState();
    _attachScrollController(widget.controller);
    if (widget.targetMessageId != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        unawaited(_focusTargetMessage());
      });
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (widget.controller == null) {
      _attachScrollController(PrimaryScrollController.maybeOf(context));
    }
  }

  @override
  void didUpdateWidget(covariant _ConversationHistory oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.messages, widget.messages)) {
      for (var index = 0; index < widget.messages.length; index++) {
        final group = _assistantResponseVersions(widget.messages, index);
        if (group.length < 2 || group.first.id != widget.messages[index].id) {
          continue;
        }
        final previousIds = oldWidget.messages
            .where((message) => message.role == ChatMessageRole.assistant)
            .map((message) => message.id)
            .toSet();
        if (group.any((message) => !previousIds.contains(message.id))) {
          _selectedResponseVersions.remove(group.first.id);
        }
      }
    }
    if (oldWidget.controller != widget.controller) {
      _attachScrollController(widget.controller);
    }
    final targetChanged =
        oldWidget.targetMessageId != widget.targetMessageId ||
        oldWidget.targetRequestId != widget.targetRequestId;
    final targetBecameAvailable =
        widget.targetMessageId != null &&
        !oldWidget.messages.any(
          (message) => message.id == widget.targetMessageId,
        ) &&
        widget.messages.any((message) => message.id == widget.targetMessageId);
    if (widget.targetMessageId != null &&
        (targetChanged || targetBecameAvailable)) {
      unawaited(_focusTargetMessage());
    } else if (_historyAdvanced(oldWidget)) {
      _scrollToLatestAfterLayout();
    }
  }

  @override
  void dispose() {
    _stopMiddleScrolling(updateUi: false);
    _targetHighlightTimer?.cancel();
    _targetAttemptGeneration++;
    _attachScrollController(null);
    super.dispose();
  }

  void _attachScrollController(ScrollController? controller) {
    if (identical(_historyController, controller)) return;
    if (_isMiddleScrolling) _stopMiddleScrolling(updateUi: false);
    _historyController?.removeListener(_updateFollowState);
    _historyController = controller;
    _historyController?.addListener(_updateFollowState);
  }

  bool _historyAdvanced(_ConversationHistory oldWidget) {
    if (oldWidget.showAssistantLoading != widget.showAssistantLoading ||
        oldWidget.messages.length != widget.messages.length) {
      return true;
    }
    return oldWidget.messages.isNotEmpty &&
        !identical(oldWidget.messages.last, widget.messages.last);
  }

  void _updateFollowState() {
    final controller = _historyController;
    if (controller == null || !controller.hasClients) return;
    final position = controller.position;
    if (!position.hasContentDimensions) return;
    _followLatestMessage = position.extentAfter <= _latestMessageThreshold;
  }

  void _scrollToLatestAfterLayout() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final controller = _historyController;
      if (!mounted ||
          !_followLatestMessage ||
          controller == null ||
          !controller.hasClients) {
        return;
      }
      final position = controller.position;
      if (position.hasContentDimensions) {
        position.jumpTo(position.maxScrollExtent);
      }
    });
  }

  Future<void> _focusTargetMessage() async {
    final messageId = widget.targetMessageId;
    final requestId = widget.targetRequestId;
    if (messageId == null || _handledTargetRequestId == requestId) return;
    if (!widget.messagesLoaded) return;
    await WidgetsBinding.instance.endOfFrame;
    if (!mounted ||
        widget.targetMessageId != messageId ||
        widget.targetRequestId != requestId) {
      return;
    }

    final targetIndex = widget.messages.indexWhere(
      (message) => message.id == messageId,
    );
    if (targetIndex < 0) {
      _finishTargetRequest(messageId, requestId, found: false);
      return;
    }

    final targetVersions = _assistantResponseVersions(
      widget.messages,
      targetIndex,
    );
    var visualTargetIndex = targetIndex;
    if (targetVersions.length > 1) {
      final targetVersionIndex = targetVersions.indexWhere(
        (message) => message.id == messageId,
      );
      visualTargetIndex -= targetVersionIndex;
      if (_selectedResponseVersions[targetVersions.first.id] != messageId) {
        setState(
          () => _selectedResponseVersions[targetVersions.first.id] = messageId,
        );
      }
    }

    final attemptGeneration = ++_targetAttemptGeneration;
    _followLatestMessage = false;
    for (var attempt = 0; attempt < 24; attempt++) {
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted ||
          attemptGeneration != _targetAttemptGeneration ||
          widget.targetMessageId != messageId ||
          widget.targetRequestId != requestId) {
        return;
      }

      final targetSliverIndex = visualTargetIndex + 1;
      final targetLayoutOffset = _targetLayoutOffset(targetSliverIndex);
      final controller = _historyController;
      if (targetLayoutOffset != null &&
          controller != null &&
          controller.hasClients &&
          controller.position.hasContentDimensions) {
        final position = controller.position;
        final targetOffset =
            targetLayoutOffset - position.viewportDimension * 0.42;
        final nextOffset = targetOffset
            .clamp(position.minScrollExtent, position.maxScrollExtent)
            .toDouble();
        final disableAnimations = MediaQuery.of(context).disableAnimations;
        if (disableAnimations) {
          controller.jumpTo(nextOffset);
        } else {
          await controller.animateTo(
            nextOffset,
            duration: const Duration(milliseconds: 260),
            curve: Curves.easeOutCubic,
          );
        }
        if (!mounted || attemptGeneration != _targetAttemptGeneration) return;
        _targetHighlightTimer?.cancel();
        setState(() => _focusedMessageId = messageId);
        _targetHighlightTimer = Timer(const Duration(seconds: 3), () {
          if (mounted && _focusedMessageId == messageId) {
            setState(() => _focusedMessageId = null);
          }
        });
        _finishTargetRequest(messageId, requestId, found: true);
        return;
      }

      if (!_estimateScrollTowardMessage(visualTargetIndex + 1)) break;
    }
    _finishTargetRequest(messageId, requestId, found: false);
  }

  double? _targetLayoutOffset(int targetSliverIndex) {
    final renderObject = _historySliverKey.currentContext?.findRenderObject();
    if (renderObject is! RenderSliverMultiBoxAdaptor) return null;
    var child = renderObject.firstChild;
    while (child != null) {
      if (renderObject.indexOf(child) == targetSliverIndex) {
        final parentData = child.parentData! as SliverMultiBoxAdaptorParentData;
        return parentData.layoutOffset;
      }
      child = renderObject.childAfter(child);
    }
    return null;
  }

  bool _estimateScrollTowardMessage(int targetSliverIndex) {
    final controller = _historyController;
    if (controller == null || !controller.hasClients) return false;
    final position = controller.position;
    if (!position.hasContentDimensions) return false;

    final renderObject = _historySliverKey.currentContext?.findRenderObject();
    if (renderObject is RenderSliverMultiBoxAdaptor &&
        renderObject.firstChild != null) {
      final firstChild = renderObject.firstChild!;
      final lastChild = renderObject.lastChild ?? firstChild;
      final firstData =
          firstChild.parentData! as SliverMultiBoxAdaptorParentData;
      final lastData = lastChild.parentData! as SliverMultiBoxAdaptorParentData;
      final firstIndex = firstData.index;
      final lastIndex = lastData.index;
      final firstOffset = firstData.layoutOffset;
      final lastOffset = lastData.layoutOffset;
      if (firstIndex != null &&
          lastIndex != null &&
          firstOffset != null &&
          lastOffset != null) {
        final itemCount = lastIndex - firstIndex + 1;
        final measuredExtent = lastOffset + lastChild.size.height - firstOffset;
        final averageExtent = measuredExtent / itemCount;
        if (averageExtent.isFinite && averageExtent > 0) {
          final estimatedOffset =
              firstOffset +
              (targetSliverIndex - firstIndex) * averageExtent -
              position.viewportDimension * 0.42;
          final nextOffset = estimatedOffset
              .clamp(position.minScrollExtent, position.maxScrollExtent)
              .toDouble();
          if ((nextOffset - position.pixels).abs() >= 1) {
            controller.jumpTo(nextOffset);
            return true;
          }
        }
      }
    }

    final itemCount =
        widget.messages.length + (widget.showAssistantLoading ? 1 : 0) + 2;
    if (itemCount < 2) return false;
    final fraction = targetSliverIndex / (itemCount - 1);
    final estimatedOffset = position.maxScrollExtent * fraction;
    if ((estimatedOffset - position.pixels).abs() < 1) return false;
    controller.jumpTo(estimatedOffset);
    return true;
  }

  void _finishTargetRequest(
    String messageId,
    int requestId, {
    required bool found,
  }) {
    if (_handledTargetRequestId == requestId) return;
    _handledTargetRequestId = requestId;
    widget.onTargetHandled?.call(messageId, found);
  }

  void _handlePointerDown(PointerDownEvent event) {
    if (event.kind != PointerDeviceKind.mouse ||
        (event.buttons & kMiddleMouseButton) == 0) {
      return;
    }
    if (_isMiddleScrolling) {
      _stopMiddleScrolling();
      return;
    }

    final controller = _historyController;
    if (controller == null || !controller.hasClients) return;
    _middleScrollPointer = event.pointer;
    _middleScrollAnchorY = event.position.dy;
    _middleScrollPointerY = event.position.dy;
    _middleScrollClock = Stopwatch()..start();
    GestureBinding.instance.pointerRouter.addGlobalRoute(
      _handleGlobalPointerEvent,
    );
    setState(() => _isMiddleScrolling = true);
    _middleScrollTimer = Timer.periodic(
      const Duration(milliseconds: 16),
      _advanceMiddleScroll,
    );
  }

  void _handleGlobalPointerEvent(PointerEvent event) {
    if (!_isMiddleScrolling || event.pointer != _middleScrollPointer) return;
    if (event is PointerMoveEvent || event is PointerHoverEvent) {
      _middleScrollPointerY = event.position.dy;
    } else if (event is PointerCancelEvent) {
      _stopMiddleScrolling();
    }
  }

  void _handlePointerSignal(PointerSignalEvent event) {
    if (event is PointerScrollEvent && _isMiddleScrolling) {
      _stopMiddleScrolling();
    }
  }

  void _advanceMiddleScroll(Timer _) {
    final controller = _historyController;
    final clock = _middleScrollClock;
    final anchorY = _middleScrollAnchorY;
    final pointerY = _middleScrollPointerY;
    if (controller == null ||
        !controller.hasClients ||
        clock == null ||
        anchorY == null ||
        pointerY == null) {
      return;
    }

    final position = controller.position;
    if (!position.hasContentDimensions) return;
    final elapsedMicroseconds = clock.elapsedMicroseconds;
    clock.reset();
    if (elapsedMicroseconds == 0) return;

    final distance = pointerY - anchorY;
    final activeDistance = distance.abs() - _autoScrollDeadZone;
    if (activeDistance <= 0) return;
    final speed = (distance.sign * activeDistance * _autoScrollSpeedFactor)
        .clamp(-_maximumAutoScrollSpeed, _maximumAutoScrollSpeed)
        .toDouble();
    final offset =
        (position.pixels +
                speed * elapsedMicroseconds / Duration.microsecondsPerSecond)
            .clamp(position.minScrollExtent, position.maxScrollExtent)
            .toDouble();
    if (offset != position.pixels) controller.jumpTo(offset);
  }

  void _stopMiddleScrolling({bool updateUi = true}) {
    if (!_isMiddleScrolling) return;
    _middleScrollTimer?.cancel();
    _middleScrollTimer = null;
    _middleScrollClock?.stop();
    _middleScrollClock = null;
    _middleScrollPointer = null;
    _middleScrollAnchorY = null;
    _middleScrollPointerY = null;
    GestureBinding.instance.pointerRouter.removeGlobalRoute(
      _handleGlobalPointerEvent,
    );
    if (updateUi && mounted) {
      setState(() => _isMiddleScrolling = false);
    } else {
      _isMiddleScrolling = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final messages = widget.messages;
    final showAssistantLoading = widget.showAssistantLoading;
    final assistantModelLabel = widget.assistantModelLabel;
    final onRetryResponse = widget.onRetryResponse;
    final providerId = widget.providerId;
    final controller =
        widget.controller ?? PrimaryScrollController.maybeOf(context);
    final conversationStyle = OpenChatConversationStyle.of(context);
    final hasStreamingAssistant =
        messages.isNotEmpty &&
        messages.last.role == ChatMessageRole.assistant &&
        messages.last.status == ChatMessageStatus.streaming;
    final appendLoadingMessage = showAssistantLoading && !hasStreamingAssistant;

    return MouseRegion(
      cursor: _isMiddleScrolling
          ? SystemMouseCursors.allScroll
          : MouseCursor.defer,
      child: Listener(
        onPointerDown: _handlePointerDown,
        onPointerSignal: _handlePointerSignal,
        child: Scrollbar(
          controller: controller,
          thumbVisibility: true,
          trackVisibility: true,
          interactive: true,
          thickness: 6,
          radius: const Radius.circular(8),
          scrollbarOrientation: ScrollbarOrientation.right,
          child: SelectionArea(
            child: CustomScrollView(
              primary: controller == null,
              controller: controller,
              slivers: [
                SliverList(
                  key: _historySliverKey,
                  delegate: SliverChildBuilderDelegate(
                    (context, index) {
                      if (index == 0) {
                        return const SizedBox(
                          key: ValueKey<String>(
                            'conversation-history-top-space',
                          ),
                          height: 18,
                        );
                      }
                      final messageIndex = index - 1;
                      if (appendLoadingMessage &&
                          messageIndex == messages.length) {
                        final previousMessage = messages.isEmpty
                            ? null
                            : messages.last;
                        final messageGap = previousMessage == null
                            ? 0.0
                            : previousMessage.role == ChatMessageRole.user
                            ? 2.0
                            : 18.0;
                        return Center(
                          child: ConstrainedBox(
                            constraints: BoxConstraints(
                              maxWidth: conversationStyle.maxWidth,
                            ),
                            child: Padding(
                              padding: EdgeInsets.only(top: messageGap),
                              child: AssistantMessageSkeleton(
                                modelLabel: assistantModelLabel,
                                providerId: providerId,
                              ),
                            ),
                          ),
                        );
                      }
                      if (messageIndex >= messages.length) {
                        return const SizedBox(
                          key: ValueKey<String>(
                            'conversation-history-bottom-space',
                          ),
                          height: 24,
                        );
                      }
                      final indexedMessage = messages[messageIndex];
                      final responseVersions = _assistantResponseVersions(
                        messages,
                        messageIndex,
                      );
                      var responseVersionIndex = 0;
                      var message = indexedMessage;
                      if (responseVersions.length > 1) {
                        final versionOffset = responseVersions.indexWhere(
                          (version) => version.id == indexedMessage.id,
                        );
                        final groupStartIndex = messageIndex - versionOffset;
                        if (messageIndex != groupStartIndex) {
                          return SizedBox.shrink(
                            key: ValueKey<String>(
                              'conversation-response-version-${indexedMessage.id}',
                            ),
                          );
                        }
                        final selectedId =
                            _selectedResponseVersions[responseVersions
                                .first
                                .id];
                        responseVersionIndex = responseVersions.indexWhere(
                          (version) => version.id == selectedId,
                        );
                        if (responseVersionIndex < 0) {
                          responseVersionIndex = responseVersions.length - 1;
                        }
                        message = responseVersions[responseVersionIndex];
                      }
                      final startsNewDay =
                          messageIndex == 0 ||
                          !_isSameDay(
                            message.createdAt,
                            messages[messageIndex - 1].createdAt,
                          );
                      final previousMessage = messageIndex == 0
                          ? null
                          : messages[messageIndex - 1];
                      final messageGap = previousMessage == null || startsNewDay
                          ? 0.0
                          : previousMessage.role == ChatMessageRole.user &&
                                message.role == ChatMessageRole.assistant
                          ? 2.0
                          : 18.0;

                      final isFocusedMessage = message.id == _focusedMessageId;
                      return Center(
                        key: ValueKey<String>(message.id),
                        child: AnimatedContainer(
                          duration: MediaQuery.of(context).disableAnimations
                              ? Duration.zero
                              : const Duration(milliseconds: 220),
                          decoration: BoxDecoration(
                            border: Border.all(
                              color: isFocusedMessage
                                  ? Theme.of(context).colorScheme.primary
                                  : Colors.transparent,
                              width: 1.5,
                            ),
                            borderRadius: BorderRadius.circular(8),
                          ),
                          child: ConstrainedBox(
                            constraints: BoxConstraints(
                              maxWidth: conversationStyle.maxWidth,
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                if (messageIndex > 0)
                                  SizedBox(
                                    height: startsNewDay ? 18 : messageGap,
                                  ),
                                if (startsNewDay) ...[
                                  _ConversationDateLabel(
                                    createdAt: message.createdAt,
                                  ),
                                  const SizedBox(height: 12),
                                ],
                                switch (message.role) {
                                  ChatMessageRole.user => _UserMessage(
                                    message: message,
                                    onBranchMessage: widget.onBranchMessage,
                                  ),
                                  ChatMessageRole.assistant => AssistantMessage(
                                    message: message,
                                    isSavedOutput: widget.savedOutputMessageIds
                                        .contains(message.id),
                                    isSavingSavedOutput: widget
                                        .pendingSavedOutputMessageIds
                                        .contains(message.id),
                                    onToggleSavedOutput:
                                        widget.onToggleSavedOutput == null
                                        ? null
                                        : () => widget.onToggleSavedOutput!(
                                            message,
                                          ),
                                    modelLabel:
                                        message.modelId ?? assistantModelLabel,
                                    providerId:
                                        message.providerId ?? providerId,
                                    responseVersionIndex:
                                        responseVersions.length > 1
                                        ? responseVersionIndex
                                        : null,
                                    responseVersionCount:
                                        responseVersions.length > 1
                                        ? responseVersions.length
                                        : null,
                                    onSelectResponseVersion:
                                        responseVersions.length > 1
                                        ? (index) => setState(
                                            () =>
                                                _selectedResponseVersions[responseVersions
                                                        .first
                                                        .id] =
                                                    responseVersions[index].id,
                                          )
                                        : null,
                                    onRetry:
                                        responseVersions.length == 1 &&
                                            _canRetryMessage(messageIndex)
                                        ? () => onRetryResponse?.call(message)
                                        : null,
                                  ),
                                },
                                if (message.role == ChatMessageRole.assistant &&
                                    message.status !=
                                        ChatMessageStatus.streaming)
                                  FileChangesSummaryCard(
                                    message: message,
                                    latestChanges: widget.latestFileChanges,
                                    onViewChanges: widget.onOpenFileChanges,
                                  ),
                              ],
                            ),
                          ),
                        ),
                      );
                    },
                    childCount:
                        messages.length + (appendLoadingMessage ? 1 : 0) + 2,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  bool _isSameDay(DateTime? left, DateTime? right) {
    if (left == null || right == null) return left == null && right == null;
    return DateUtils.isSameDay(left.toLocal(), right.toLocal());
  }

  bool _canRetryMessage(int index) {
    final messages = widget.messages;
    if (widget.onRetryResponse == null ||
        index == 0 ||
        index != messages.length - 1) {
      return false;
    }
    final message = messages[index];
    return message.role == ChatMessageRole.assistant &&
        message.status != ChatMessageStatus.streaming &&
        messages[index - 1].role == ChatMessageRole.user;
  }
}

List<ChatFileChange> _mergeConversationFileChanges(
  List<ChatMessage> messages,
  List<ChatFileChange> latestChanges,
) {
  final changesById = <String, ChatFileChange>{};
  for (final message in messages) {
    if (message.role != ChatMessageRole.assistant) continue;
    for (final activity in message.toolActivities) {
      for (final change in activity.fileChanges) {
        changesById[change.id] = change;
      }
    }
  }
  for (final change in latestChanges) {
    if (changesById.containsKey(change.id)) changesById[change.id] = change;
  }
  return changesById.values.toList(growable: false);
}

List<ChatMessage> _assistantResponseVersions(
  List<ChatMessage> messages,
  int index,
) {
  if (index < 0 || index >= messages.length) return const <ChatMessage>[];
  if (messages[index].role != ChatMessageRole.assistant) {
    return <ChatMessage>[messages[index]];
  }
  var start = index;
  while (start > 0 && messages[start - 1].role == ChatMessageRole.assistant) {
    start--;
  }
  var end = index;
  while (end + 1 < messages.length &&
      messages[end + 1].role == ChatMessageRole.assistant) {
    end++;
  }
  return messages.sublist(start, end + 1);
}

class _FileChangesButton extends StatelessWidget {
  const _FileChangesButton({required this.changes, required this.onPressed});

  final List<ChatFileChange> changes;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final active = changes
        .where((change) => change.status == ChatFileChangeState.active)
        .toList(growable: false);
    final added = active.fold<int>(
      0,
      (total, change) => total + (change.addedLines ?? 0),
    );
    final removed = active.fold<int>(
      0,
      (total, change) => total + (change.removedLines ?? 0),
    );
    final countsUnavailable = active.any(
      (change) => change.addedLines == null || change.removedLines == null,
    );
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    return OutlinedButton.icon(
      onPressed: onPressed,
      icon: const Icon(LucideIcons.fileDiff, size: 16),
      label: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            l10n.fileChangesOpenButton,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(width: 8),
          if (countsUnavailable)
            Text(
              l10n.fileChangesSomeCountsUnavailable,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: palette.secondaryText),
            )
          else ...[
            Text(
              '+$added',
              style: TextStyle(
                color: Theme.of(context).colorScheme.tertiary,
                fontWeight: FontWeight.w600,
              ),
            ),
            const SizedBox(width: 6),
            Text(
              '-$removed',
              style: TextStyle(
                color: Theme.of(context).colorScheme.error,
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
        ],
      ),
      style: OutlinedButton.styleFrom(
        foregroundColor: palette.text,
        visualDensity: VisualDensity.compact,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        side: BorderSide(color: palette.border),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(9)),
      ),
    );
  }
}

class _ConversationDateLabel extends StatelessWidget {
  const _ConversationDateLabel({required this.createdAt});

  final DateTime? createdAt;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final localDate = createdAt?.toLocal();
    if (localDate == null) return const SizedBox.shrink();
    final label = DateUtils.isSameDay(localDate, DateTime.now())
        ? l10n.today
        : DateFormat.yMMMMd(l10n.localeName).format(localDate);

    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 20),
      child: Center(
        child: Text(
          label,
          style: TextStyle(
            color: OpenChatPalette.of(context).secondaryText,
            fontSize: 12,
            fontWeight: FontWeight.w400,
            height: 18 / 12,
          ),
        ),
      ),
    );
  }
}

class _UserMessage extends StatelessWidget {
  const _UserMessage({required this.message, this.onBranchMessage});

  final ChatMessage message;
  final Future<void> Function(ChatMessage message, String editedContent)?
  onBranchMessage;

  Future<void> _editAndBranch(BuildContext context) async {
    final editedContent = await showDialog<String>(
      context: context,
      builder: (context) =>
          _EditConversationBranchDialog(initialContent: message.content),
    );
    if (editedContent == null || !context.mounted) return;
    await onBranchMessage?.call(message, editedContent.trim());
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final conversationStyle = OpenChatConversationStyle.of(context);
    final l10n = context.openchatL10n;
    final timestamp = switch (message.createdAt) {
      final createdAt? => DateFormat.Hm(
        l10n.localeName,
      ).format(createdAt.toLocal()),
      null => null,
    };

    return Semantics(
      label: l10n.userMessage,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Align(
            alignment: Alignment.centerRight,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Flexible(
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth:
                            conversationStyle.maxWidth *
                            (540 / OpenChatSpacing.conversationMaxWidth),
                      ),
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 16,
                          vertical: 12,
                        ),
                        decoration: BoxDecoration(
                          color: palette.composer,
                          borderRadius: const BorderRadius.only(
                            topLeft: Radius.circular(16),
                            topRight: Radius.circular(16),
                            bottomLeft: Radius.circular(16),
                            bottomRight: Radius.circular(4),
                          ),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (message.content.isNotEmpty)
                              Text(
                                message.content,
                                style: TextStyle(
                                  color: palette.text,
                                  fontFamily: conversationStyle.fontFamily,
                                  fontSize: 15,
                                  fontWeight: FontWeight.w400,
                                  height: 22 / 15,
                                ),
                              ),
                            if (message.attachments.isNotEmpty) ...[
                              if (message.content.isNotEmpty)
                                const SizedBox(height: 10),
                              ChatAttachmentGallery(
                                attachments: message.attachments,
                                palette: palette,
                                preferredImageWidth: 280,
                                imageHeight: 200,
                              ),
                            ],
                          ],
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 2),
          Align(
            alignment: Alignment.centerRight,
            child: Padding(
              padding: const EdgeInsets.only(right: 20),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (timestamp != null) ...[
                    Text(
                      timestamp,
                      textAlign: TextAlign.right,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                        fontWeight: FontWeight.w400,
                        height: 18 / 12,
                      ),
                    ),
                    const SizedBox(width: 4),
                  ],
                  if (message.content.trim().isNotEmpty)
                    CopyMessageButton(content: message.content),
                  if (onBranchMessage != null)
                    IconButton(
                      tooltip: l10n.conversationBranchEditTitle,
                      visualDensity: VisualDensity.compact,
                      onPressed: () => _editAndBranch(context),
                      icon: const Icon(LucideIcons.gitBranch, size: 15),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _EditConversationBranchDialog extends StatefulWidget {
  const _EditConversationBranchDialog({required this.initialContent});

  final String initialContent;

  @override
  State<_EditConversationBranchDialog> createState() =>
      _EditConversationBranchDialogState();
}

class _EditConversationBranchDialogState
    extends State<_EditConversationBranchDialog> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.initialContent,
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _submit() => Navigator.of(context).pop(_controller.text);

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(context.openchatL10n.conversationBranchEditTitle),
    content: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 520),
      child: TextField(
        controller: _controller,
        autofocus: true,
        minLines: 2,
        maxLines: 8,
        decoration: InputDecoration(
          labelText: context.openchatL10n.conversationBranchEditLabel,
          border: const OutlineInputBorder(),
        ),
        onSubmitted: (_) => _submit(),
      ),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.of(context).pop(),
        child: Text(context.openchatL10n.cancel),
      ),
      FilledButton(
        onPressed: _submit,
        child: Text(context.openchatL10n.conversationBranchStart),
      ),
    ],
  );
}

class _NewConversationEmptyState extends StatelessWidget {
  const _NewConversationEmptyState({
    required this.isLoading,
    required this.title,
    required this.description,
    this.onOpenConnections,
    this.onRetryModels,
  });

  final bool isLoading;
  final String title;
  final String? description;
  final VoidCallback? onOpenConnections;
  final VoidCallback? onRetryModels;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return LayoutBuilder(
      builder: (context, constraints) {
        final descriptionText = constraints.maxHeight >= 400
            ? description
            : null;
        final compactTitle = constraints.maxHeight < 280;
        final titleFontSize = compactTitle
            ? OpenChatTypography.componentTitle + 2
            : OpenChatTypography.welcomeTitle;

        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            if (isLoading) ...[
              const SizedBox.square(
                dimension: 24,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(height: 16),
            ],
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 600),
              child: Text(
                title,
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: palette.text,
                  fontSize: titleFontSize,
                  fontWeight: FontWeight.w600,
                  height: 24 / titleFontSize,
                ),
              ),
            ),
            if (descriptionText != null) ...[
              const SizedBox(height: 10),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 480),
                child: Text(
                  descriptionText,
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.body,
                    height: 22 / OpenChatTypography.body,
                  ),
                ),
              ),
            ],
            if (onOpenConnections != null || onRetryModels != null) ...[
              SizedBox(
                height: constraints.maxHeight < 120
                    ? 4
                    : descriptionText != null
                    ? 20
                    : 12,
              ),
              Wrap(
                alignment: WrapAlignment.center,
                spacing: 8,
                runSpacing: 8,
                children: [
                  if (onOpenConnections != null)
                    FilledButton.icon(
                      onPressed: onOpenConnections,
                      icon: const Icon(LucideIcons.link),
                      label: Text(context.openchatL10n.connections),
                    ),
                  if (onRetryModels != null)
                    OutlinedButton.icon(
                      onPressed: onRetryModels,
                      icon: const Icon(LucideIcons.refreshCw),
                      label: Text(context.openchatL10n.retry),
                    ),
                ],
              ),
            ],
          ],
        );
      },
    );
  }
}

class _ConversationHeader extends StatefulWidget {
  const _ConversationHeader({
    required this.showHistoryButton,
    required this.title,
    required this.productWorkspaceName,
    required this.conversationId,
    required this.titleEditRequestId,
    required this.onRenameConversation,
    required this.onTitleEditFinished,
    required this.onOpenHistory,
    this.historyButtonTooltip,
  });

  final bool showHistoryButton;
  final String title;
  final String? productWorkspaceName;
  final String? conversationId;
  final String? titleEditRequestId;
  final Future<void> Function(String conversationId, String title)?
  onRenameConversation;
  final VoidCallback? onTitleEditFinished;
  final VoidCallback onOpenHistory;
  final String? historyButtonTooltip;

  @override
  State<_ConversationHeader> createState() => _ConversationHeaderState();
}

class _ConversationHeaderState extends State<_ConversationHeader> {
  late final TextEditingController _titleController;
  late final FocusNode _titleFocusNode;
  bool _isEditing = false;
  bool _isSaving = false;
  bool _titleHasFocus = false;

  @override
  void initState() {
    super.initState();
    _titleController = TextEditingController(text: widget.title);
    _titleFocusNode = FocusNode();
    _isEditing =
        widget.conversationId != null &&
        widget.titleEditRequestId == widget.conversationId;
    if (_isEditing) _focusTitleInput();
  }

  @override
  void didUpdateWidget(covariant _ConversationHeader oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.conversationId != widget.conversationId) {
      _isEditing =
          widget.conversationId != null &&
          widget.titleEditRequestId == widget.conversationId;
      _isSaving = false;
      _titleController.text = widget.title;
      if (_isEditing) _focusTitleInput();
      return;
    }
    if (!_isEditing && oldWidget.title != widget.title) {
      _titleController.text = widget.title;
    }
    if (widget.conversationId != null &&
        widget.titleEditRequestId == widget.conversationId &&
        oldWidget.titleEditRequestId != widget.titleEditRequestId) {
      _beginTitleEdit();
    }
  }

  void _focusTitleInput() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_isEditing) return;
      _titleFocusNode.requestFocus();
      _titleController.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _titleController.text.length,
      );
    });
  }

  void _beginTitleEdit() {
    if (widget.conversationId == null || widget.onRenameConversation == null) {
      return;
    }
    setState(() {
      _isEditing = true;
      _titleController.text = widget.title;
    });
    _focusTitleInput();
  }

  void _cancelTitleEdit() {
    if (_isSaving) return;
    setState(() {
      _isEditing = false;
      _titleController.text = widget.title;
    });
    widget.onTitleEditFinished?.call();
  }

  Future<void> _saveTitle() async {
    if (_isSaving) return;
    final conversationId = widget.conversationId;
    final saveTitle = widget.onRenameConversation;
    final normalizedTitle = _titleController.text.trim();
    if (conversationId == null || saveTitle == null) return;
    if (normalizedTitle.isEmpty) {
      showOpenChatToast(
        context,
        context.openchatL10n.conversationTitleRequired,
        type: OpenChatToastType.error,
      );
      return;
    }
    if (normalizedTitle == widget.title) {
      _cancelTitleEdit();
      return;
    }

    setState(() => _isSaving = true);
    try {
      await saveTitle(conversationId, normalizedTitle);
      if (!mounted || widget.conversationId != conversationId) return;
      setState(() {
        _isSaving = false;
        _isEditing = false;
      });
      widget.onTitleEditFinished?.call();
    } on Object {
      if (mounted && widget.conversationId == conversationId) {
        setState(() => _isSaving = false);
      }
    }
  }

  @override
  void dispose() {
    _titleController.dispose();
    _titleFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final canRename =
        widget.conversationId != null && widget.onRenameConversation != null;
    final showTitle =
        widget.conversationId != null || widget.title != l10n.conversationTitle;
    final workspaceName = widget.productWorkspaceName?.trim();
    final hasWorkspaceContext =
        workspaceName != null && workspaceName.isNotEmpty;
    final scaledHeaderHeight = MediaQuery.textScalerOf(context).scale(22) + 12;

    return ConstrainedBox(
      key: const ValueKey<String>('conversation-header'),
      constraints: BoxConstraints(
        minHeight: OpenChatSpacing.conversationHeaderHeight,
        maxHeight: hasWorkspaceContext
            ? math.max(
                math.max(
                  OpenChatSpacing.conversationHeaderHeight,
                  scaledHeaderHeight,
                ),
                MediaQuery.textScalerOf(context).scale(13) * 2 + 34,
              )
            : showTitle
            ? math.max(
                math.max(
                  OpenChatSpacing.conversationHeaderHeight,
                  scaledHeaderHeight,
                ),
                MediaQuery.textScalerOf(context).scale(14) * (20 / 14) + 18,
              )
            : math.max(
                OpenChatSpacing.conversationHeaderHeight,
                scaledHeaderHeight,
              ),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Row(
          children: [
            if (widget.showHistoryButton) ...[
              Tooltip(
                message: widget.historyButtonTooltip ?? l10n.historyOpen,
                child: IconButton(
                  onPressed: widget.onOpenHistory,
                  icon: const Icon(LucideIcons.menu),
                ),
              ),
              const SizedBox(width: 12),
            ],
            if (showTitle) ...[
              const Icon(LucideIcons.messageCircle, size: 18),
              const SizedBox(width: 10),
            ],
            Expanded(
              child: _isEditing
                  ? Align(
                      alignment: Alignment.centerLeft,
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 420),
                        child: Row(
                          children: [
                            Expanded(
                              child: Focus(
                                onKeyEvent: (node, event) {
                                  if (event is KeyDownEvent &&
                                      event.logicalKey ==
                                          LogicalKeyboardKey.escape) {
                                    _cancelTitleEdit();
                                    return KeyEventResult.handled;
                                  }
                                  return KeyEventResult.ignored;
                                },
                                child: TextField(
                                  controller: _titleController,
                                  focusNode: _titleFocusNode,
                                  enabled: !_isSaving,
                                  maxLines: 1,
                                  textInputAction: TextInputAction.done,
                                  onSubmitted: (_) => unawaited(_saveTitle()),
                                  style: TextStyle(
                                    color: palette.text,
                                    fontSize: 14,
                                    fontWeight: FontWeight.w500,
                                    height: 20 / 14,
                                  ),
                                  decoration: InputDecoration(
                                    labelText: l10n.conversationTitle,
                                    isDense: true,
                                    filled: true,
                                    fillColor: palette.composer,
                                    contentPadding: const EdgeInsets.symmetric(
                                      horizontal: 12,
                                      vertical: 9,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                            IconButton(
                              tooltip: l10n.save,
                              constraints: const BoxConstraints.tightFor(
                                width: 44,
                                height: 44,
                              ),
                              padding: EdgeInsets.zero,
                              onPressed: _isSaving
                                  ? null
                                  : () => unawaited(_saveTitle()),
                              icon: _isSaving
                                  ? const SizedBox.square(
                                      dimension: 16,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  : const Icon(LucideIcons.check, size: 18),
                            ),
                            IconButton(
                              tooltip: l10n.cancel,
                              constraints: const BoxConstraints.tightFor(
                                width: 44,
                                height: 44,
                              ),
                              padding: EdgeInsets.zero,
                              onPressed: _isSaving ? null : _cancelTitleEdit,
                              icon: const Icon(LucideIcons.x, size: 18),
                            ),
                          ],
                        ),
                      ),
                    )
                  : Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Semantics(
                          button: true,
                          enabled: canRename,
                          label: widget.title,
                          onTap: canRename ? _beginTitleEdit : null,
                          child: ExcludeSemantics(
                            child: InkWell(
                              onTap: canRename ? _beginTitleEdit : null,
                              onFocusChange: (focused) {
                                if (_titleHasFocus == focused) return;
                                setState(() => _titleHasFocus = focused);
                              },
                              borderRadius: BorderRadius.circular(4),
                              child: DecoratedBox(
                                decoration: BoxDecoration(
                                  color: _titleHasFocus ? palette.hover : null,
                                  borderRadius: BorderRadius.circular(4),
                                ),
                                child: Padding(
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 3,
                                    vertical: 3,
                                  ),
                                  child: Text(
                                    showTitle ? widget.title : '',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      color: palette.text,
                                      fontSize: 13,
                                      fontWeight: FontWeight.w500,
                                      height: 20 / 13,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                        if (hasWorkspaceContext)
                          Semantics(
                            label: l10n.conversationWorkspaceContext(
                              workspaceName,
                            ),
                            child: ExcludeSemantics(
                              child: Padding(
                                padding: const EdgeInsetsDirectional.only(
                                  start: 4,
                                  top: 1,
                                ),
                                child: Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                    Icon(
                                      LucideIcons.folder,
                                      size: 12,
                                      color: palette.secondaryIcon,
                                    ),
                                    const SizedBox(width: 4),
                                    Flexible(
                                      child: Text(
                                        workspaceName,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                          color: palette.secondaryText,
                                          fontSize: OpenChatTypography.metadata,
                                          height: 14 / 10,
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
