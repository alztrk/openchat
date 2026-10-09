import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/chat_attachment.dart';
import 'package:openchat/features/chat/domain/chatgpt_connection.dart';
import 'package:openchat/features/chat/domain/model_favorite.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/presentation/widgets/composer_control_style.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/features/chat/presentation/widgets/context_usage_indicator.dart';
import 'package:openchat/features/chat/presentation/widgets/model_selector.dart';

bool _usesTouchTargets(BuildContext context) {
  return switch (Theme.of(context).platform) {
    TargetPlatform.android ||
    TargetPlatform.iOS ||
    TargetPlatform.fuchsia => true,
    TargetPlatform.windows ||
    TargetPlatform.macOS ||
    TargetPlatform.linux => false,
  };
}

Color _focusRingColor(BuildContext context) {
  return OpenChatSemanticColors.of(context).focusRing;
}

class ChatComposer extends StatelessWidget {
  const ChatComposer({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    this.isLoadingModels = false,
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
    this.modelsEmptyLabel,
    this.hiddenModelKeys = const <String>{},
    this.selectedModelId,
    this.selectedModelRouteKey,
    this.onModelSelected,
    this.onModelFavoriteChanged,
    this.onFavoriteModelSelected,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    this.isSending = false,
    this.onStopMessage,
    this.modelLabel,
    this.reasoningLevel,
    this.messages = const <ChatMessage>[],
    this.conversationId,
    this.contextProviderId,
    this.contextModelId,
    this.contextSupportsTools,
    this.contextWindow,
    this.contextConnectionId,
    this.contextWorkspaceId,
    this.conversationMemoryRepository,
    this.settingsPreferences,
    this.pendingAttachments = const <ChatAttachment>[],
    this.onAddAttachments,
    this.onRemoveAttachment,
    this.attachmentsEnabled = false,
    super.key,
  });

  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool showReasoningSelector;
  final ToolPermissionMode toolPermissionMode;
  final ValueChanged<ToolPermissionMode>? onToolPermissionModeChanged;
  final bool isLoadingModels;
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
  final String? modelsEmptyLabel;
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
  final ValueChanged<String?>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final List<ChatMessage> messages;
  final String? conversationId;
  final String? contextProviderId;
  final String? contextModelId;
  final bool? contextSupportsTools;
  final int? contextWindow;
  final String? contextConnectionId;
  final String? contextWorkspaceId;
  final ConversationMemoryRepository? conversationMemoryRepository;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final ValueChanged<String>? onRemoveAttachment;
  final bool attachmentsEnabled;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return LayoutBuilder(
      builder: (context, constraints) {
        final narrow =
            constraints.maxWidth <= 600 ||
            MediaQuery.textScalerOf(context).scale(16) > 18;
        final touchTargets = _usesTouchTargets(context);
        final focusRing = _focusRingColor(context);
        final horizontalPadding = narrow ? 12.0 : 16.0;

        return ConstrainedBox(
          constraints: BoxConstraints(minHeight: narrow ? 104 : 100),
          child: Focus(
            skipTraversal: true,
            child: Builder(
              builder: (focusContext) {
                final hasFocus = Focus.of(focusContext).hasFocus;
                return Container(
                  decoration: BoxDecoration(
                    color: palette.composer,
                    border: Border.all(
                      color: hasFocus ? focusRing : palette.controlBorder,
                      width: hasFocus ? 2 : 1,
                    ),
                    borderRadius: BorderRadius.circular(narrow ? 16 : 20),
                  ),
                  padding: EdgeInsets.fromLTRB(
                    horizontalPadding,
                    narrow ? 12 : 15,
                    horizontalPadding,
                    narrow ? 12 : 13,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      _GoalSlashInput(
                        controller: controller,
                        onSendMessage: onSendMessage,
                        canSendMessage: canSendMessage,
                        isSending: isSending,
                        onStopMessage: onStopMessage,
                        hasPendingAttachments: pendingAttachments.isNotEmpty,
                        focusRing: focusRing,
                        palette: palette,
                      ),
                      if (pendingAttachments.isNotEmpty) ...[
                        const SizedBox(height: 12),
                        ChatAttachmentGallery(
                          attachments: pendingAttachments,
                          palette: palette,
                          preferredImageWidth: 160,
                          imageHeight: 104,
                          onRemoveAttachment: onRemoveAttachment,
                        ),
                      ],
                      SizedBox(height: narrow ? 14 : 12),
                      _ComposerActions(
                        controller: controller,
                        onSendMessage: onSendMessage,
                        canSendMessage: canSendMessage,
                        showReasoningSelector: showReasoningSelector,
                        toolPermissionMode: toolPermissionMode,
                        onToolPermissionModeChanged:
                            onToolPermissionModeChanged,
                        isLoadingModels: isLoadingModels,
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
                        reasoningOptions: reasoningOptions,
                        onReasoningSelected: onReasoningSelected,
                        isSending: isSending,
                        onStopMessage: onStopMessage,
                        modelLabel: modelLabel,
                        reasoningLevel: reasoningLevel,
                        messages: messages,
                        conversationId: conversationId,
                        contextProviderId: contextProviderId,
                        contextModelId: contextModelId,
                        contextSupportsTools: contextSupportsTools,
                        contextWindow: contextWindow,
                        contextConnectionId: contextConnectionId,
                        contextWorkspaceId: contextWorkspaceId,
                        conversationMemoryRepository:
                            conversationMemoryRepository,
                        settingsPreferences: settingsPreferences,
                        pendingAttachments: pendingAttachments,
                        onAddAttachments: onAddAttachments,
                        attachmentsEnabled: attachmentsEnabled,
                        narrow: narrow,
                        touchTargets: touchTargets,
                        palette: palette,
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        );
      },
    );
  }
}

class _GoalSlashInput extends StatefulWidget {
  const _GoalSlashInput({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.isSending,
    required this.onStopMessage,
    required this.hasPendingAttachments,
    required this.focusRing,
    required this.palette,
  });

  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final bool hasPendingAttachments;
  final Color focusRing;
  final OpenChatPalette palette;

  @override
  State<_GoalSlashInput> createState() => _GoalSlashInputState();
}

class _GoalSlashInputState extends State<_GoalSlashInput> {
  bool _showGoalSuggestion = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_handleTextChanged);
    _showGoalSuggestion = _matchesGoalCommand(widget.controller.text);
  }

  @override
  void didUpdateWidget(covariant _GoalSlashInput oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_handleTextChanged);
      widget.controller.addListener(_handleTextChanged);
      _updateSuggestionVisibility();
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleTextChanged);
    super.dispose();
  }

  bool _matchesGoalCommand(String value) {
    if (value.contains(RegExp(r'\s'))) return false;
    return '/goal'.startsWith(value.toLowerCase()) && value.startsWith('/');
  }

  void _handleTextChanged() => _updateSuggestionVisibility();

  void _updateSuggestionVisibility() {
    final shouldShow = _matchesGoalCommand(widget.controller.text);
    if (shouldShow == _showGoalSuggestion || !mounted) return;
    setState(() => _showGoalSuggestion = shouldShow);
  }

  void _insertGoalCommand() {
    widget.controller.value = const TextEditingValue(
      text: '/goal ',
      selection: TextSelection.collapsed(offset: 6),
    );
    _updateSuggestionVisibility();
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    if (_showGoalSuggestion &&
        (event.logicalKey == LogicalKeyboardKey.arrowDown ||
            event.logicalKey == LogicalKeyboardKey.tab ||
            event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter)) {
      _insertGoalCommand();
      return KeyEventResult.handled;
    }
    if (widget.isSending && event.logicalKey == LogicalKeyboardKey.escape) {
      widget.onStopMessage?.call();
      return KeyEventResult.handled;
    }
    if ((event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter) &&
        !HardwareKeyboard.instance.isShiftPressed) {
      if (widget.canSendMessage &&
          (widget.controller.text.trim().isNotEmpty ||
              widget.hasPendingAttachments)) {
        widget.onSendMessage();
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    return Focus(
      skipTraversal: true,
      onKeyEvent: _handleKeyEvent,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_showGoalSuggestion) ...[
            Semantics(
              button: true,
              label: '${l10n.goalSlashCommand}. ${l10n.goalCommandDescription}',
              child: Material(
                color: theme.colorScheme.surfaceContainerHigh,
                borderRadius: BorderRadius.circular(OpenChatRadii.menu),
                child: InkWell(
                  onTap: _insertGoalCommand,
                  borderRadius: BorderRadius.circular(OpenChatRadii.menu),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 9,
                    ),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          l10n.goalSlashCommand,
                          style: theme.textTheme.labelMedium?.copyWith(
                            color: widget.palette.text,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            l10n.goalCommandDescription,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: widget.palette.secondaryText,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
            const SizedBox(height: 10),
          ],
          Semantics(
            label: l10n.messageHint,
            child: TextField(
              controller: widget.controller,
              minLines: 1,
              maxLines: 3,
              textCapitalization: TextCapitalization.sentences,
              textInputAction: TextInputAction.newline,
              cursorColor: widget.focusRing,
              decoration: InputDecoration(
                hintText: l10n.messageHint,
                hintStyle: TextStyle(
                  color: widget.palette.secondaryText,
                  fontSize: 15,
                  fontWeight: FontWeight.w400,
                  height: 20 / 15,
                ),
                filled: false,
                border: InputBorder.none,
                enabledBorder: InputBorder.none,
                focusedBorder: InputBorder.none,
                contentPadding: EdgeInsets.zero,
                isDense: true,
                isCollapsed: true,
              ),
              style: TextStyle(
                color: widget.palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w400,
                height: 20 / 15,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ComposerActions extends StatelessWidget {
  const _ComposerActions({
    required this.controller,
    required this.onSendMessage,
    required this.canSendMessage,
    required this.showReasoningSelector,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    required this.isLoadingModels,
    required this.models,
    required this.favoriteModels,
    required this.providerId,
    required this.isChatGptConnected,
    required this.chatGptFastModeEnabled,
    required this.chatGptFastModeAvailable,
    required this.chatGptFastModeLoading,
    required this.chatGptFastModeSaving,
    required this.onChatGptFastModeChanged,
    required this.availableProviderIds,
    required this.onProviderSelected,
    required this.modelsEmptyLabel,
    this.hiddenModelKeys = const <String>{},
    required this.selectedModelId,
    required this.selectedModelRouteKey,
    required this.onModelSelected,
    required this.onModelFavoriteChanged,
    required this.onFavoriteModelSelected,
    required this.reasoningOptions,
    required this.onReasoningSelected,
    required this.isSending,
    required this.onStopMessage,
    required this.modelLabel,
    required this.reasoningLevel,
    required this.messages,
    required this.conversationId,
    required this.contextProviderId,
    required this.contextModelId,
    required this.contextSupportsTools,
    required this.contextWindow,
    required this.contextConnectionId,
    required this.contextWorkspaceId,
    required this.conversationMemoryRepository,
    required this.settingsPreferences,
    required this.pendingAttachments,
    required this.onAddAttachments,
    required this.attachmentsEnabled,
    required this.narrow,
    required this.touchTargets,
    required this.palette,
  });

  final TextEditingController controller;
  final VoidCallback onSendMessage;
  final bool canSendMessage;
  final bool showReasoningSelector;
  final ToolPermissionMode toolPermissionMode;
  final ValueChanged<ToolPermissionMode>? onToolPermissionModeChanged;
  final bool isLoadingModels;
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
  final String? modelsEmptyLabel;
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
  final ValueChanged<String?>? onReasoningSelected;
  final bool isSending;
  final VoidCallback? onStopMessage;
  final String? modelLabel;
  final String? reasoningLevel;
  final List<ChatMessage> messages;
  final String? conversationId;
  final String? contextProviderId;
  final String? contextModelId;
  final bool? contextSupportsTools;
  final int? contextWindow;
  final String? contextConnectionId;
  final String? contextWorkspaceId;
  final ConversationMemoryRepository? conversationMemoryRepository;
  final SettingsPreferences? settingsPreferences;
  final List<ChatAttachment> pendingAttachments;
  final VoidCallback? onAddAttachments;
  final bool attachmentsEnabled;
  final bool narrow;
  final bool touchTargets;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final dark = Theme.of(context).brightness == Brightness.dark;
    final iconRoot = dark ? 'assets/icons/dark' : 'assets/icons';
    final focusRing = _focusRingColor(context);
    final semantic = OpenChatSemanticColors.of(context);
    final primary = semantic.primary;
    final primaryForeground = semantic.primaryForeground;
    final selectedModelLabel = modelLabel?.trim();
    final modelSelector = ModelSelector(
      label: selectedModelLabel == null || selectedModelLabel.isEmpty
          ? l10n.modelSelection
          : selectedModelLabel,
      palette: palette,
      compact: touchTargets,
      width: narrow ? 144 : 152,
      models: models,
      favoriteModels: favoriteModels,
      selectedModelId: selectedModelId,
      selectedModelRouteKey: selectedModelRouteKey,
      providerId: providerId,
      isChatGptConnected: isChatGptConnected,
      chatGptFastModeEnabled: chatGptFastModeEnabled,
      chatGptFastModeAvailable: chatGptFastModeAvailable,
      chatGptFastModeLoading: chatGptFastModeLoading,
      chatGptFastModeSaving: chatGptFastModeSaving,
      onChatGptFastModeChanged: onChatGptFastModeChanged,
      availableProviderIds: availableProviderIds,
      onProviderSelected: onProviderSelected,
      isLoadingModels: isLoadingModels,
      emptyModelsLabel:
          modelsEmptyLabel ?? context.openchatL10n.noModelsAvailable,
      onSelected: onModelSelected,
      onFavoriteChanged: onModelFavoriteChanged,
      onFavoriteSelected: onFavoriteModelSelected,
      hiddenModelKeys: hiddenModelKeys,
    );
    final reasoningSelector = showReasoningSelector
        ? _ReasoningSelector(
            level: reasoningLevel,
            iconRoot: iconRoot,
            palette: palette,
            defaultHint: l10n.reasoningDefaultHint,
            compact: touchTargets,
            width: narrow ? 144 : 152,
            options: reasoningOptions,
            onSelected: onReasoningSelected,
          )
        : null;
    final toolPermissionSelector = _ToolPermissionSelector(
      mode: toolPermissionMode,
      palette: palette,
      compact: touchTargets,
      width: narrow ? 168 : 176,
      hasSelectedModel: selectedModelId != null,
      supportsToolCalls: contextSupportsTools,
      onSelected: onToolPermissionModeChanged,
    );
    final attachmentSize = touchTargets ? 44.0 : 36.0;
    final sendSize = touchTargets ? 44.0 : 40.0;
    final attachmentButton = Tooltip(
      message: attachmentsEnabled
          ? l10n.attachFile
          : l10n.attachmentsUnavailable,
      child: Semantics(
        button: true,
        enabled: attachmentsEnabled,
        label: attachmentsEnabled
            ? l10n.attachFile
            : l10n.attachmentsUnavailable,
        onTap: attachmentsEnabled ? onAddAttachments : null,
        child: ExcludeSemantics(
          child: SizedBox.square(
            dimension: attachmentSize,
            child: OutlinedButton(
              onPressed: attachmentsEnabled ? onAddAttachments : null,
              style: composerControlStyle(
                palette,
                width: attachmentSize,
                height: attachmentSize,
                compact: touchTargets,
                focusColor: focusRing,
              ),
              child: Icon(
                LucideIcons.paperclip,
                size: 18,
                color: attachmentsEnabled
                    ? palette.secondaryIcon
                    : palette.disabledIcon,
              ),
            ),
          ),
        ),
      ),
    );
    final contextIndicator = ContextUsageIndicator(
      controller: controller,
      messages: messages,
      providerId: contextProviderId,
      modelId: contextModelId,
      supportsTools: contextSupportsTools,
      contextWindow: contextWindow,
      repository: conversationMemoryRepository,
      conversationId: conversationId,
      isSending: isSending,
      connectionId: contextConnectionId,
      workspaceId: contextWorkspaceId,
      settingsPreferences: settingsPreferences,
      toolPermissionMode: toolPermissionMode,
      pendingAttachments: pendingAttachments,
    );
    final sendButton = ValueListenableBuilder<TextEditingValue>(
      valueListenable: controller,
      builder: (context, value, _) {
        final canAttemptSend =
            canSendMessage &&
            (value.text.trim().isNotEmpty || pendingAttachments.isNotEmpty);
        final actionEnabled = isSending
            ? onStopMessage != null
            : canAttemptSend;
        final onPressed = isSending
            ? onStopMessage
            : canAttemptSend
            ? () {
                if (controller.text.trim().isNotEmpty ||
                    pendingAttachments.isNotEmpty) {
                  onSendMessage();
                }
              }
            : null;
        final tooltip = isSending ? l10n.stop : l10n.send;
        return Tooltip(
          message: tooltip,
          child: Semantics(
            button: true,
            enabled: actionEnabled,
            label: tooltip,
            onTap: actionEnabled ? onPressed : null,
            child: ExcludeSemantics(
              child: SizedBox.square(
                dimension: sendSize,
                child: IconButton(
                  onPressed: onPressed,
                  tooltip: null,
                  padding: EdgeInsets.zero,
                  iconSize: 18,
                  constraints: BoxConstraints.tightFor(
                    width: sendSize,
                    height: sendSize,
                  ),
                  style: IconButton.styleFrom(
                    backgroundColor: actionEnabled ? primary : palette.selected,
                    foregroundColor: actionEnabled
                        ? primaryForeground
                        : palette.disabledForeground,
                    disabledBackgroundColor: palette.selected,
                    disabledForegroundColor: palette.disabledForeground,
                    minimumSize: Size.square(sendSize),
                    maximumSize: Size.square(sendSize),
                    padding: EdgeInsets.zero,
                    tapTargetSize: touchTargets
                        ? MaterialTapTargetSize.padded
                        : MaterialTapTargetSize.shrinkWrap,
                    shape: const CircleBorder(),
                  ),
                  icon: isSending
                      ? const Icon(LucideIcons.square)
                      : const Icon(LucideIcons.arrowUp, size: 18),
                ),
              ),
            ),
          ),
        );
      },
    );
    final fullAccessIndicator =
        toolPermissionMode == ToolPermissionMode.fullAccess
        ? Tooltip(
            message: l10n.toolPermissionFullAccessDescription,
            child: Semantics(
              label:
                  '${l10n.toolPermissionFullAccess}. ${l10n.toolPermissionFullAccessDescription}',
              child: Container(
                constraints: const BoxConstraints(minHeight: 36),
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 7,
                ),
                decoration: BoxDecoration(
                  color: palette.warning.withValues(alpha: 0.12),
                  border: Border.all(color: palette.warning),
                  borderRadius: BorderRadius.circular(OpenChatRadii.control),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      LucideIcons.shieldAlert,
                      size: 16,
                      color: palette.warning,
                    ),
                    const SizedBox(width: 6),
                    Text(
                      l10n.toolPermissionFullAccess,
                      style: TextStyle(
                        color: palette.warning,
                        fontSize: OpenChatTypography.metadata,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          )
        : null;
    final primaryControls = narrow
        ? Wrap(
            spacing: 8,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              attachmentButton,
              modelSelector,
              ?fullAccessIndicator,
              sendButton,
            ],
          )
        : Row(
            children: [
              attachmentButton,
              const SizedBox(width: 8),
              modelSelector,
              const Spacer(),
              ?fullAccessIndicator,
              const SizedBox(width: 8),
              sendButton,
            ],
          );
    final advancedControls = Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [toolPermissionSelector, ?reasoningSelector, contextIndicator],
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        primaryControls,
        _ComposerOptionsDisclosure(child: advancedControls),
      ],
    );
  }
}

class _ComposerOptionsDisclosure extends StatefulWidget {
  const _ComposerOptionsDisclosure({required this.child});

  final Widget child;

  @override
  State<_ComposerOptionsDisclosure> createState() =>
      _ComposerOptionsDisclosureState();
}

class _ComposerOptionsDisclosureState
    extends State<_ComposerOptionsDisclosure> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final palette = OpenChatPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Semantics(
          expanded: _expanded,
          child: TextButton.icon(
            onPressed: () => setState(() => _expanded = !_expanded),
            icon: Icon(
              _expanded ? LucideIcons.chevronUp : LucideIcons.slidersHorizontal,
              size: 16,
            ),
            label: Text(l10n.moreOptions),
            style: TextButton.styleFrom(
              foregroundColor: palette.secondaryText,
              minimumSize: const Size(40, 40),
              padding: const EdgeInsets.symmetric(horizontal: 8),
            ),
          ),
        ),
        AnimatedSize(
          duration: reducedMotion
              ? Duration.zero
              : const Duration(milliseconds: 160),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topLeft,
          child: _expanded
              ? Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: widget.child,
                )
              : const SizedBox(width: double.infinity),
        ),
      ],
    );
  }
}

class _ReasoningSelector extends StatelessWidget {
  const _ReasoningSelector({
    required this.level,
    required this.iconRoot,
    required this.palette,
    required this.defaultHint,
    required this.compact,
    required this.width,
    required this.options,
    required this.onSelected,
  });

  final String? level;
  final String iconRoot;
  final OpenChatPalette palette;
  final String defaultHint;
  final bool compact;
  final double width;
  final List<String> options;
  final ValueChanged<String?>? onSelected;

  @override
  Widget build(BuildContext context) {
    final currentLevel = level;
    final l10n = context.openchatL10n;
    return Tooltip(
      message: currentLevel == null
          ? defaultHint
          : _reasoningLabel(context, currentLevel),
      child: OpenChatSelect<String?>(
        options: [
          OpenChatSelectOption<String?>(
            value: null,
            label: l10n.reasoningDefault,
          ),
          for (final option in options)
            OpenChatSelectOption<String?>(
              value: option,
              label: _reasoningLabel(context, option),
            ),
        ],
        value: currentLevel,
        onChanged: onSelected,
        palette: palette,
        width: width,
        menuWidth: width,
        height: composerControlHeight(context),
        compact: compact,
        triggerStyle: composerControlStyle(
          palette,
          width: width,
          height: composerControlHeight(context),
          compact: compact,
          focusColor: _focusRingColor(context),
        ),
        selectedContent: Row(
          children: [
            SvgPicture.asset(
              '$iconRoot/brain.svg',
              width: 16,
              height: 16,
              colorFilter: ColorFilter.mode(
                palette.secondaryText,
                BlendMode.srcIn,
              ),
              excludeFromSemantics: true,
            ),
            const SizedBox(width: 4),
            Flexible(
              child: Text(
                currentLevel == null
                    ? l10n.reasoningDefault
                    : _reasoningLabel(context, currentLevel),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: palette.text,
                  fontSize: 12,
                  fontWeight: FontWeight.w500,
                  height: 16 / 12,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ToolPermissionSelector extends StatelessWidget {
  const _ToolPermissionSelector({
    required this.mode,
    required this.palette,
    required this.compact,
    required this.width,
    required this.hasSelectedModel,
    required this.supportsToolCalls,
    required this.onSelected,
  });

  final ToolPermissionMode mode;
  final OpenChatPalette palette;
  final bool compact;
  final double width;
  final bool hasSelectedModel;
  final bool? supportsToolCalls;
  final ValueChanged<ToolPermissionMode>? onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final warning = OpenChatSemanticColors.of(context).warning;
    final label = switch (mode) {
      ToolPermissionMode.plan => l10n.toolPermissionPlan,
      ToolPermissionMode.requireApproval => l10n.toolPermissionRequireApproval,
      ToolPermissionMode.approveSafeOperations =>
        l10n.toolPermissionApproveSafeOperations,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
    };
    final description = switch (mode) {
      ToolPermissionMode.plan => l10n.toolPermissionPlanDescription,
      ToolPermissionMode.requireApproval =>
        l10n.toolPermissionRequireApprovalDescription,
      ToolPermissionMode.approveSafeOperations =>
        l10n.toolPermissionApproveSafeOperationsDescription,
      ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccessDescription,
    };
    final modeIcon = switch (mode) {
      ToolPermissionMode.plan => LucideIcons.search,
      ToolPermissionMode.requireApproval => LucideIcons.hand,
      ToolPermissionMode.approveSafeOperations => LucideIcons.shieldCheck,
      ToolPermissionMode.fullAccess => LucideIcons.shieldAlert,
    };
    final modeColor = mode == ToolPermissionMode.fullAccess
        ? warning
        : palette.secondaryIcon;
    final selector = OpenChatSelect<ToolPermissionMode>(
      options: [
        for (final option in ToolPermissionMode.values)
          OpenChatSelectOption<ToolPermissionMode>(
            value: option,
            label: switch (option) {
              ToolPermissionMode.plan => l10n.toolPermissionPlan,
              ToolPermissionMode.requireApproval =>
                l10n.toolPermissionRequireApproval,
              ToolPermissionMode.approveSafeOperations =>
                l10n.toolPermissionApproveSafeOperations,
              ToolPermissionMode.fullAccess => l10n.toolPermissionFullAccess,
            },
            description: switch (option) {
              ToolPermissionMode.plan => l10n.toolPermissionPlanDescription,
              ToolPermissionMode.requireApproval =>
                l10n.toolPermissionRequireApprovalDescription,
              ToolPermissionMode.approveSafeOperations =>
                l10n.toolPermissionApproveSafeOperationsDescription,
              ToolPermissionMode.fullAccess =>
                l10n.toolPermissionFullAccessDescription,
            },
            icon: switch (option) {
              ToolPermissionMode.plan => LucideIcons.search,
              ToolPermissionMode.requireApproval => LucideIcons.hand,
              ToolPermissionMode.approveSafeOperations =>
                LucideIcons.shieldCheck,
              ToolPermissionMode.fullAccess => LucideIcons.shieldAlert,
            },
            iconColor: option == ToolPermissionMode.fullAccess
                ? warning
                : palette.secondaryText,
            descriptionColor: option == ToolPermissionMode.fullAccess
                ? warning
                : palette.secondaryText,
            selectedColor: option == ToolPermissionMode.fullAccess
                ? warning
                : null,
            textStyle: option == ToolPermissionMode.fullAccess
                ? TextStyle(color: warning, fontSize: 13)
                : null,
          ),
      ],
      value: mode,
      onChanged: onSelected,
      palette: palette,
      width: width,
      menuWidth: 440,
      height: composerControlHeight(context),
      compact: compact,
      triggerStyle: composerControlStyle(
        palette,
        width: width,
        height: composerControlHeight(context),
        compact: compact,
        leftPadding: 10,
        rightPadding: 10,
        focusColor: _focusRingColor(context),
      ),
      leadingIcon: modeIcon,
      leadingIconColor: modeColor,
      leadingIconGap: 5,
      selectedContent: Text(
        label,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.labelLarge?.copyWith(
          color: mode == ToolPermissionMode.fullAccess ? warning : palette.text,
          fontSize: 13,
          fontWeight: FontWeight.w500,
          height: 18 / 13,
        ),
      ),
    );
    final capabilityNotice = !hasSelectedModel
        ? null
        : switch (supportsToolCalls) {
            true => null,
            false => l10n.selectedModelDoesNotSupportToolCalls,
            null => l10n.selectedModelToolSupportUnknown,
          };
    return Tooltip(
      message: capabilityNotice == null
          ? description
          : '$description\n$capabilityNotice',
      child: selector,
    );
  }
}

String _reasoningLabel(BuildContext context, String value) {
  final l10n = context.openchatL10n;
  return switch (value) {
    'minimal' => l10n.reasoningMinimal,
    'low' => l10n.reasoningLow,
    'medium' => l10n.reasoningMedium,
    'high' => l10n.reasoningHigh,
    'xhigh' => l10n.reasoningExtraHigh,
    'max' => l10n.reasoningMax,
    'ultra' => l10n.reasoningUltra,
    _ => value,
  };
}
