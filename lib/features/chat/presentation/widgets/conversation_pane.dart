import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:intl/intl.dart';

import '../../../../app/openchat_theme.dart';
import '../../../../app/openchat_toast.dart';
import '../../domain/chat_message.dart';
import '../../domain/chatgpt_connection.dart';
import '../../domain/model_favorite.dart';
import '../../../../l10n/openchat_localizations.dart';
import '../../../../platform/windows/window_controls.dart';
import '../../../settings/data/settings_preferences.dart';
import 'assistant_message.dart';
import 'chat_composer.dart';
import 'window_control_bar.dart';

class ConversationPane extends StatelessWidget {
  const ConversationPane({
    required this.messageController,
    required this.showHistoryButton,
    required this.onOpenHistory,
    this.historyButtonTooltip,
    required this.onSendMessage,
    this.onRetryResponse,
    this.onStopMessage,
    this.canSendMessage = false,
    this.isSending = false,
    this.isLoadingModels = false,
    this.models = const <ChatGptModel>[],
    this.favoriteModels = const <FavoriteModel>[],
    this.providerId = 'chatgpt',
    this.onProviderSelected,
    this.selectedModelId,
    this.onModelSelected,
    this.onModelFavoriteChanged,
    this.onFavoriteModelSelected,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    this.messages = const <ChatMessage>[],
    this.messagesLoading = false,
    this.showAssistantLoading = false,
    this.messagesErrorDescription,
    this.conversationTitle,
    this.conversationId,
    this.titleEditRequestId,
    this.onRenameConversation,
    this.onConversationTitleEditFinished,
    this.selectedModelLabel,
    this.modelsEmptyLabel,
    this.assistantModelLabel,
    this.reasoningLevel,
    this.toolPermissionMode = ToolPermissionMode.requireApproval,
    this.onToolPermissionModeChanged,
    this.showWindowControls,
    this.messageScrollController,
    super.key,
  });

  final TextEditingController messageController;
  final ScrollController? messageScrollController;
  final bool showHistoryButton;
  final VoidCallback onOpenHistory;
  final String? historyButtonTooltip;
  final VoidCallback onSendMessage;
  final ValueChanged<ChatMessage>? onRetryResponse;
  final VoidCallback? onStopMessage;
  final bool canSendMessage;
  final bool isSending;
  final bool isLoadingModels;
  final List<ChatGptModel> models;
  final List<FavoriteModel> favoriteModels;
  final String providerId;
  final ValueChanged<String>? onProviderSelected;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final void Function(
    String providerId,
    String modelId,
    String displayName,
    bool isFavorite,
  )?
  onModelFavoriteChanged;
  final ValueChanged<FavoriteModel>? onFavoriteModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final List<ChatMessage> messages;
  final bool messagesLoading;
  final bool showAssistantLoading;
  final String? messagesErrorDescription;
  final String? conversationTitle;
  final String? conversationId;
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
  final bool? showWindowControls;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final hasSelectedModel = selectedModelLabel?.trim().isNotEmpty ?? false;

    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 640;
        final horizontalPadding = compact
            ? OpenChatSpacing.compactPageHorizontal
            : OpenChatSpacing.pageHorizontal;

        return Column(
          children: [
            _ConversationHeader(
              showHistoryButton: showHistoryButton,
              title: conversationTitle ?? l10n.conversationTitle,
              conversationId: conversationId,
              titleEditRequestId: titleEditRequestId,
              onRenameConversation: onRenameConversation,
              onTitleEditFinished: onConversationTitleEditFinished,
              onOpenHistory: onOpenHistory,
              historyButtonTooltip: historyButtonTooltip,
              showWindowControls:
                  showWindowControls ?? OpenChatWindowControls.isSupported,
            ),
            Expanded(
              child: messagesErrorDescription != null
                  ? _ConversationMessageError(
                      description: messagesErrorDescription!,
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
                  : messages.isEmpty && !showAssistantLoading
                  ? _NewConversationEmptyState(
                      title: l10n.emptyChatWelcomeTitle,
                      description: l10n.emptyChatWelcomeBody,
                    )
                  : _ConversationHistory(
                      messages: messages,
                      showAssistantLoading: showAssistantLoading,
                      assistantModelLabel: assistantModelLabel,
                      onRetryResponse: onRetryResponse,
                      controller: messageScrollController,
                      providerId: providerId,
                    ),
            ),
            Padding(
              padding: EdgeInsets.fromLTRB(
                horizontalPadding,
                0,
                horizontalPadding,
                24,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
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
                    onProviderSelected: onProviderSelected,
                    modelsEmptyLabel: modelsEmptyLabel,
                    selectedModelId: selectedModelId,
                    onModelSelected: onModelSelected,
                    onModelFavoriteChanged: onModelFavoriteChanged,
                    onFavoriteModelSelected: onFavoriteModelSelected,
                    reasoningLevel: reasoningLevel,
                    reasoningOptions: reasoningOptions,
                    onReasoningSelected: onReasoningSelected,
                    showReasoningSelector: hasSelectedModel,
                    toolPermissionMode: toolPermissionMode,
                    onToolPermissionModeChanged: onToolPermissionModeChanged,
                  ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }
}

class _ConversationMessageError extends StatelessWidget {
  const _ConversationMessageError({required this.description});

  final String description;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 440),
          child: Text(
            description,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodyMedium
                ?.copyWith(color: OpenChatPalette.of(context).secondaryText),
          ),
        ),
      ),
    );
  }
}

class _ConversationHistory extends StatelessWidget {
  const _ConversationHistory({
    required this.messages,
    required this.showAssistantLoading,
    required this.assistantModelLabel,
    required this.onRetryResponse,
    required this.controller,
    required this.providerId,
  });

  final List<ChatMessage> messages;
  final bool showAssistantLoading;
  final String? assistantModelLabel;
  final ValueChanged<ChatMessage>? onRetryResponse;
  final ScrollController? controller;
  final String providerId;

  @override
  Widget build(BuildContext context) {
    final conversationStyle = OpenChatConversationStyle.of(context);
    final hasStreamingAssistant =
        messages.isNotEmpty &&
        messages.last.role == ChatMessageRole.assistant &&
        messages.last.status == ChatMessageStatus.streaming;
    final appendLoadingMessage = showAssistantLoading && !hasStreamingAssistant;

    return Scrollbar(
      controller: controller,
      thumbVisibility: true,
      trackVisibility: true,
      interactive: true,
      thickness: 6,
      radius: const Radius.circular(8),
      scrollbarOrientation: ScrollbarOrientation.right,
      child: SelectionArea(
        child: ListView.builder(
          primary: controller == null,
          controller: controller,
          padding: const EdgeInsets.fromLTRB(
            OpenChatSpacing.pageHorizontal,
            18,
            OpenChatSpacing.pageHorizontal,
            24,
          ),
          itemCount: messages.length + (appendLoadingMessage ? 1 : 0),
          itemBuilder: (context, index) {
            if (index == messages.length) {
              final previousMessage = messages.isEmpty ? null : messages.last;
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
            final message = messages[index];
            final startsNewDay =
                index == 0 ||
                !_isSameDay(messages[index - 1].createdAt, message.createdAt);
            final previousMessage = index == 0 ? null : messages[index - 1];
            final messageGap = previousMessage == null || startsNewDay
                ? 0.0
                : previousMessage.role == ChatMessageRole.user &&
                      message.role == ChatMessageRole.assistant
                ? 2.0
                : 18.0;

            return Center(
              child: ConstrainedBox(
                constraints: BoxConstraints(
                  maxWidth: conversationStyle.maxWidth,
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (index > 0)
                      SizedBox(height: startsNewDay ? 18 : messageGap),
                    if (startsNewDay) ...[
                      _ConversationDateLabel(createdAt: message.createdAt),
                      const SizedBox(height: 12),
                    ],
                    KeyedSubtree(
                      key: ValueKey<String>(message.id),
                      child: switch (message.role) {
                        ChatMessageRole.user => _UserMessage(message: message),
                        ChatMessageRole.assistant => AssistantMessage(
                          message: message,
                          modelLabel: assistantModelLabel,
                          providerId: providerId,
                          onRetry: _canRetryMessage(index)
                              ? () => onRetryResponse?.call(message)
                              : null,
                        ),
                      },
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      ),
    );
  }

  bool _isSameDay(DateTime? left, DateTime? right) {
    if (left == null || right == null) return left == null && right == null;
    return DateUtils.isSameDay(left.toLocal(), right.toLocal());
  }

  bool _canRetryMessage(int index) {
    if (onRetryResponse == null || index == 0 || index != messages.length - 1) {
      return false;
    }
    final message = messages[index];
    return message.role == ChatMessageRole.assistant &&
        message.status != ChatMessageStatus.streaming &&
        messages[index - 1].role == ChatMessageRole.user;
  }
}

class _ConversationDateLabel extends StatelessWidget {
  const _ConversationDateLabel({required this.createdAt});

  final DateTime? createdAt;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final localDate = createdAt?.toLocal();
    final label =
        localDate == null || DateUtils.isSameDay(localDate, DateTime.now())
        ? l10n.today
        : DateFormat.yMMMMd(l10n.localeName).format(localDate);

    return SizedBox(
      height: 20,
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
  const _UserMessage({required this.message});

  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final conversationStyle = OpenChatConversationStyle.of(context);
    final l10n = context.openchatL10n;
    final timestamp = message.createdAt == null
        ? l10n.unavailableTime
        : DateFormat.Hm(l10n.localeName).format(message.createdAt!.toLocal());

    return Column(
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
                      child: Text(
                        message.content,
                        style: TextStyle(
                          color: palette.text,
                          fontFamily: conversationStyle.fontFamily,
                          fontSize: 15 * conversationStyle.textScale,
                          fontWeight: FontWeight.w400,
                          height: 22 / 15,
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Padding(
                  padding: const EdgeInsets.only(top: 10),
                  child: Tooltip(
                    message: l10n.userMessage,
                    child: Icon(
                      Icons.person_outline_rounded,
                      size: 20,
                      color: palette.secondaryIcon,
                      semanticLabel: l10n.userMessage,
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
                CopyMessageButton(content: message.content),
                const SizedBox(width: 28),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _NewConversationEmptyState extends StatelessWidget {
  const _NewConversationEmptyState({
    required this.title,
    required this.description,
  });

  final String title;
  final String description;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              title,
              textAlign: TextAlign.center,
              style: TextStyle(
                color: palette.text,
                fontSize: 26,
                fontWeight: FontWeight.w600,
                height: 34 / 26,
              ),
            ),
            const SizedBox(height: 10),
            Text(
              description,
              textAlign: TextAlign.center,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: 15,
                fontWeight: FontWeight.w400,
                height: 24 / 15,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ConversationHeader extends StatefulWidget {
  const _ConversationHeader({
    required this.showHistoryButton,
    required this.title,
    required this.conversationId,
    required this.titleEditRequestId,
    required this.onRenameConversation,
    required this.onTitleEditFinished,
    required this.onOpenHistory,
    this.historyButtonTooltip,
    required this.showWindowControls,
  });

  final bool showHistoryButton;
  final String title;
  final String? conversationId;
  final String? titleEditRequestId;
  final Future<void> Function(String conversationId, String title)?
  onRenameConversation;
  final VoidCallback? onTitleEditFinished;
  final VoidCallback onOpenHistory;
  final String? historyButtonTooltip;
  final bool showWindowControls;

  @override
  State<_ConversationHeader> createState() => _ConversationHeaderState();
}

class _ConversationHeaderState extends State<_ConversationHeader> {
  late final TextEditingController _titleController;
  late final FocusNode _titleFocusNode;
  bool _isEditing = false;
  bool _isSaving = false;

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

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onPanStart: OpenChatWindowControls.isSupported
          ? (_) => unawaited(OpenChatWindowControls.startDragging())
          : null,
      child: SizedBox(
        height: 68,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: OpenChatSpacing.pageHorizontal,
          ),
          child: Row(
            children: [
              if (widget.showHistoryButton) ...[
                Tooltip(
                  message: widget.historyButtonTooltip ?? l10n.historyOpen,
                  child: IconButton(
                    onPressed: widget.onOpenHistory,
                    icon: const Icon(Icons.menu_rounded),
                  ),
                ),
                const SizedBox(width: 12),
              ],
              SvgPicture.asset(
                Theme.of(context).brightness == Brightness.dark
                    ? 'assets/icons/conversation/dark.svg'
                    : 'assets/icons/conversation/light.svg',
                width: 18,
                height: 18,
                excludeFromSemantics: true,
              ),
              const SizedBox(width: 10),
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
                                      isDense: true,
                                      filled: true,
                                      fillColor: palette.composer,
                                      contentPadding:
                                          const EdgeInsets.symmetric(
                                            horizontal: 12,
                                            vertical: 9,
                                          ),
                                      border: OutlineInputBorder(
                                        borderRadius: BorderRadius.circular(8),
                                        borderSide: BorderSide(
                                          color: palette.border,
                                        ),
                                      ),
                                      enabledBorder: OutlineInputBorder(
                                        borderRadius: BorderRadius.circular(8),
                                        borderSide: BorderSide(
                                          color: palette.border,
                                        ),
                                      ),
                                      focusedBorder: OutlineInputBorder(
                                        borderRadius: BorderRadius.circular(8),
                                        borderSide: BorderSide(
                                          color: palette.accent,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                              IconButton(
                                tooltip: l10n.save,
                                visualDensity: VisualDensity.compact,
                                constraints: const BoxConstraints.tightFor(
                                  width: 34,
                                  height: 36,
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
                                    : const Icon(Icons.check_rounded, size: 18),
                              ),
                              IconButton(
                                tooltip: l10n.cancel,
                                visualDensity: VisualDensity.compact,
                                constraints: const BoxConstraints.tightFor(
                                  width: 34,
                                  height: 36,
                                ),
                                padding: EdgeInsets.zero,
                                onPressed: _isSaving ? null : _cancelTitleEdit,
                                icon: const Icon(Icons.close_rounded, size: 18),
                              ),
                            ],
                          ),
                        ),
                      )
                    : InkWell(
                        onTap: canRename ? _beginTitleEdit : null,
                        borderRadius: BorderRadius.circular(4),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(vertical: 4),
                          child: Text(
                            widget.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: palette.text,
                              fontSize: 16,
                              fontWeight: FontWeight.w600,
                              height: 24 / 16,
                            ),
                          ),
                        ),
                      ),
              ),
              if (widget.showWindowControls) const WindowControlBar(),
            ],
          ),
        ),
      ),
    );
  }
}
