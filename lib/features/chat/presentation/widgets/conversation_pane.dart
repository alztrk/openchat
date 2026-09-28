import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:html/parser.dart' as html_parser;
import 'package:markdown/markdown.dart' as markdown;
import 'package:intl/intl.dart';

import '../../../../app/zihora_theme.dart';
import '../../../../app/zihora_toast.dart';
import '../../domain/chat_message.dart';
import '../../domain/chatgpt_connection.dart';
import '../../domain/model_favorite.dart';
import '../../../../l10n/generated/app_localizations.dart';
import '../../../../l10n/zihora_localizations.dart';
import '../../../../platform/windows/window_controls.dart';
import '../../../settings/data/settings_preferences.dart';
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
    final l10n = context.zihoraL10n;
    final hasSelectedModel = selectedModelLabel?.trim().isNotEmpty ?? false;

    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 640;
        final horizontalPadding = compact
            ? ZihoraSpacing.compactPageHorizontal
            : ZihoraSpacing.pageHorizontal;

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
                  showWindowControls ?? ZihoraWindowControls.isSupported,
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
                ?.copyWith(color: ZihoraPalette.of(context).secondaryText),
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
  });

  final List<ChatMessage> messages;
  final bool showAssistantLoading;
  final String? assistantModelLabel;
  final ValueChanged<ChatMessage>? onRetryResponse;
  final ScrollController? controller;

  @override
  Widget build(BuildContext context) {
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
            ZihoraSpacing.pageHorizontal,
            18,
            ZihoraSpacing.pageHorizontal,
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
              return Padding(
                padding: EdgeInsets.only(top: messageGap),
                child: _AssistantMessageSkeleton(
                  modelLabel: assistantModelLabel,
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

            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (index > 0) SizedBox(height: startsNewDay ? 18 : messageGap),
                if (startsNewDay) ...[
                  _ConversationDateLabel(createdAt: message.createdAt),
                  const SizedBox(height: 12),
                ],
                KeyedSubtree(
                  key: ValueKey<String>(message.id),
                  child: switch (message.role) {
                    ChatMessageRole.user => _UserMessage(
                      message: message,
                      minHeight: index == 0 ? 70 : 62,
                      topPadding: index == 0 ? 13 : 10,
                      contentHeight: index == 0 ? 44 : 42,
                    ),
                    ChatMessageRole.assistant => _AssistantMessage(
                      message: message,
                      modelLabel: assistantModelLabel,
                      onRetry: _canRetryMessage(index)
                          ? () => onRetryResponse?.call(message)
                          : null,
                    ),
                  },
                ),
              ],
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
    final l10n = context.zihoraL10n;
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
            color: ZihoraPalette.of(context).secondaryText,
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
  const _UserMessage({
    required this.message,
    required this.minHeight,
    required this.topPadding,
    required this.contentHeight,
  });

  final ChatMessage message;
  final double minHeight;
  final double topPadding;
  final double contentHeight;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final timestamp = message.createdAt == null
        ? l10n.unavailableTime
        : DateFormat.Hm(l10n.localeName).format(message.createdAt!.toLocal());
    final availableWidth = MediaQuery.sizeOf(context).width;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Align(
          alignment: Alignment.centerRight,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: ConstrainedBox(
              constraints: BoxConstraints(
                maxWidth: availableWidth < 80
                    ? availableWidth
                    : availableWidth - 80,
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Flexible(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 540),
                      child: Container(
                        constraints: BoxConstraints(minHeight: minHeight),
                        padding: EdgeInsets.fromLTRB(13, topPadding, 13, 0),
                        decoration: BoxDecoration(
                          color: palette.composer,
                          borderRadius: BorderRadius.circular(10),
                        ),
                        child: ConstrainedBox(
                          constraints: BoxConstraints(minHeight: contentHeight),
                          child: Text(
                            message.content,
                            style: TextStyle(
                              color: palette.text,
                              fontSize: 15,
                              fontWeight: FontWeight.w400,
                              height: 22 / 15,
                            ),
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
                _CopyMessageButton(content: message.content),
                const SizedBox(width: 28),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _AssistantMessage extends StatelessWidget {
  const _AssistantMessage({
    required this.message,
    required this.modelLabel,
    this.onRetry,
  });

  final ChatMessage message;
  final String? modelLabel;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final localeName = l10n.localeName;
    final tokensPerSecond = message.tokensPerSecond == null
        ? l10n.unavailableValue
        : NumberFormat('0.#', localeName).format(message.tokensPerSecond);
    final outputTokens = message.outputTokens == null
        ? l10n.unavailableValue
        : NumberFormat.decimalPattern(localeName).format(message.outputTokens);
    final timestamp = message.createdAt == null
        ? l10n.unavailableTime
        : DateFormat.Hm(localeName).format(message.createdAt!.toLocal());
    final responseStatus = _responseStatusLabel(context, message);
    final waitingForFirstText =
        message.status == ChatMessageStatus.streaming &&
        message.content.trim().isEmpty;

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 780),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _AssistantModelHeader(modelLabel: modelLabel),
            for (final summary in message.reasoningSummaries.where(
              (summary) => summary.content.trim().isNotEmpty,
            )) ...[
              const SizedBox(height: 8),
              _ReasoningSummaryAccordion(summary: summary, palette: palette),
            ],
            for (final activity in message.toolActivities) ...[
              const SizedBox(height: 8),
              _ToolActivityAccordion(activity: activity, palette: palette),
            ],
            if (waitingForFirstText) ...[
              const SizedBox(height: 8),
              _AssistantResponseSkeleton(palette: palette),
            ] else ...[
              const SizedBox(height: 6),
              ConstrainedBox(
                constraints: BoxConstraints(
                  minHeight: message.content.contains('\n\n') ? 96 : 40,
                ),
                child: _AssistantResponseContent(
                  content: message.content,
                  isStreaming: message.status == ChatMessageStatus.streaming,
                  palette: palette,
                ),
              ),
              const SizedBox(height: 6),
              Wrap(
                spacing: 8,
                runSpacing: 0,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  Text(
                    l10n.responseMetadata(
                      tokensPerSecond,
                      outputTokens,
                      timestamp,
                    ),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                      fontWeight: FontWeight.w400,
                      height: 18 / 12,
                    ),
                  ),
                  if (responseStatus != null)
                    Text(
                      responseStatus,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                        fontWeight: FontWeight.w400,
                        height: 18 / 12,
                      ),
                    ),
                  if (onRetry != null)
                    IconButton(
                      tooltip: l10n.retry,
                      visualDensity: VisualDensity.compact,
                      onPressed: onRetry,
                      icon: const Icon(Icons.refresh_rounded, size: 17),
                      color: palette.secondaryIcon,
                    ),
                  _CopyMessageButton(content: message.content),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }

  String? _responseStatusLabel(BuildContext context, ChatMessage message) {
    final l10n = context.zihoraL10n;

    return switch (message.status) {
      ChatMessageStatus.streaming => null,
      ChatMessageStatus.completed => null,
      ChatMessageStatus.failed => l10n.responseFailed,
      ChatMessageStatus.stopped => l10n.responseStopped,
    };
  }
}

class _AssistantModelHeader extends StatelessWidget {
  const _AssistantModelHeader({required this.modelLabel});

  final String? modelLabel;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final displayModelLabel = modelLabel?.trim();

    return SizedBox(
      height: 20,
      child: Row(
        children: [
          SvgPicture.asset(
            'assets/icons/chatgpt.svg',
            width: 18,
            height: 18,
            colorFilter: ColorFilter.mode(
              palette.secondaryIcon,
              BlendMode.srcIn,
            ),
            excludeFromSemantics: true,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              displayModelLabel == null || displayModelLabel.isEmpty
                  ? l10n.messageModelUnavailable
                  : displayModelLabel,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: palette.accent,
                fontSize: 13,
                fontWeight: FontWeight.w600,
                height: 20 / 13,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _AssistantMessageSkeleton extends StatelessWidget {
  const _AssistantMessageSkeleton({required this.modelLabel});

  final String? modelLabel;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 780),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _AssistantModelHeader(modelLabel: modelLabel),
            const SizedBox(height: 22),
            _AssistantResponseSkeleton(palette: palette),
          ],
        ),
      ),
    );
  }
}

class _AssistantResponseSkeleton extends StatefulWidget {
  const _AssistantResponseSkeleton({required this.palette});

  final ZihoraPalette palette;

  @override
  State<_AssistantResponseSkeleton> createState() =>
      _AssistantResponseSkeletonState();
}

class _AssistantResponseSkeletonState extends State<_AssistantResponseSkeleton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _shineController;

  @override
  void initState() {
    super.initState();
    _shineController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      _shineController.stop();
    } else if (!_shineController.isAnimating) {
      _shineController.repeat();
    }
  }

  @override
  void dispose() {
    _shineController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = widget.palette;

    return Semantics(
      liveRegion: true,
      label: context.zihoraL10n.responseInProgress,
      child: ExcludeSemantics(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final width = constraints.maxWidth.isFinite
                ? constraints.maxWidth
                : 480.0;
            return AnimatedBuilder(
              animation: _shineController,
              child: SizedBox(
                width: width,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _skeletonLine(width * 0.76, palette),
                    const SizedBox(height: 9),
                    _skeletonLine(width * 0.92, palette),
                    const SizedBox(height: 9),
                    _skeletonLine(width * 0.48, palette),
                  ],
                ),
              ),
              builder: (context, child) {
                if (MediaQuery.disableAnimationsOf(context)) {
                  return child ?? const SizedBox.shrink();
                }

                final progress = _shineController.value;
                final highlight =
                    Color.lerp(palette.selected, palette.accent, 0.18) ??
                    palette.accent;
                return ShaderMask(
                  blendMode: BlendMode.srcATop,
                  shaderCallback: (bounds) => LinearGradient(
                    begin: Alignment(-1.4 + progress * 2.8, 0),
                    end: Alignment(-0.8 + progress * 2.8, 0),
                    colors: [palette.selected, highlight, palette.selected],
                    stops: const [0, 0.5, 1],
                  ).createShader(bounds),
                  child: child,
                );
              },
            );
          },
        ),
      ),
    );
  }

  Widget _skeletonLine(double width, ZihoraPalette palette) {
    return Container(
      width: width,
      height: 12,
      decoration: BoxDecoration(
        color: palette.selected,
        borderRadius: BorderRadius.circular(5),
      ),
    );
  }
}

class _ToolActivityAccordion extends StatelessWidget {
  const _ToolActivityAccordion({required this.activity, required this.palette});

  final ChatToolActivity activity;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final statusLabel = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval => l10n.toolAwaitingApproval,
      ChatToolActivityStatus.running => l10n.toolRunning,
      ChatToolActivityStatus.completed => l10n.toolCompleted,
      ChatToolActivityStatus.failed => l10n.toolFailed,
      ChatToolActivityStatus.denied => l10n.toolDenied,
      ChatToolActivityStatus.cancelled => l10n.toolCancelled,
    };
    final leading = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval => Icon(
        Icons.lock_outline_rounded,
        size: 17,
        color: palette.accent,
      ),
      ChatToolActivityStatus.running => SizedBox.square(
        dimension: 16,
        child: CircularProgressIndicator(
          strokeWidth: 1.8,
          color: palette.accent,
        ),
      ),
      ChatToolActivityStatus.completed => Icon(
        Icons.check_circle_outline_rounded,
        size: 17,
        color: palette.secondaryIcon,
      ),
      ChatToolActivityStatus.failed => Icon(
        Icons.error_outline_rounded,
        size: 17,
        color: Theme.of(context).colorScheme.error,
      ),
      ChatToolActivityStatus.denied => Icon(
        Icons.block_rounded,
        size: 17,
        color: Theme.of(context).colorScheme.error,
      ),
      ChatToolActivityStatus.cancelled => Icon(
        Icons.cancel_outlined,
        size: 17,
        color: palette.secondaryIcon,
      ),
    };
    final statusColor = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval ||
      ChatToolActivityStatus.running => palette.accent,
      ChatToolActivityStatus.failed ||
      ChatToolActivityStatus.denied => Theme.of(context).colorScheme.error,
      ChatToolActivityStatus.completed ||
      ChatToolActivityStatus.cancelled => palette.secondaryIcon,
    };
    final fileListing = activity.name == 'list_files'
        ? _ToolFileListing.fromOutput(activity.output)
        : null;
    const cardRadius = BorderRadius.all(Radius.circular(14));

    return Container(
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        color: palette.selected,
        borderRadius: cardRadius,
        border: Border.all(color: palette.border),
      ),
      child: Material(
        color: Colors.transparent,
        child: Theme(
          data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
          child: ExpansionTile(
            key: ValueKey<String>('tool-${activity.callId}'),
            initiallyExpanded:
                activity.status == ChatToolActivityStatus.running ||
                activity.status == ChatToolActivityStatus.awaitingApproval,
            tilePadding: const EdgeInsets.symmetric(horizontal: 10),
            childrenPadding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
            visualDensity: VisualDensity.compact,
            iconColor: palette.secondaryIcon,
            collapsedIconColor: palette.secondaryIcon,
            shape: const RoundedRectangleBorder(borderRadius: cardRadius),
            collapsedShape: const RoundedRectangleBorder(
              borderRadius: cardRadius,
            ),
            leading: Tooltip(
              message: statusLabel,
              child: Semantics(
                label: statusLabel,
                child: Container(
                  width: 28,
                  height: 28,
                  decoration: BoxDecoration(
                    color: palette.surface,
                    borderRadius: BorderRadius.circular(9),
                  ),
                  alignment: Alignment.center,
                  child: leading,
                ),
              ),
            ),
            title: Row(
              children: [
                Expanded(
                  child: Text(
                    _toolActivityName(activity.name, l10n),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      height: 18 / 12,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 3,
                  ),
                  decoration: BoxDecoration(
                    color: palette.surface,
                    borderRadius: BorderRadius.circular(20),
                  ),
                  child: Text(
                    statusLabel,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: statusColor,
                      fontSize: 10,
                      fontWeight: FontWeight.w500,
                      height: 14 / 10,
                    ),
                  ),
                ),
              ],
            ),
            children: [
              if (activity.name == 'list_files')
                if (fileListing == null)
                  _ToolActivityNotice(
                    message: activity.output == null
                        ? statusLabel
                        : l10n.toolListingUnavailable,
                    palette: palette,
                  )
                else
                  _ToolFileListingResult(
                    listing: fileListing,
                    locationLabel: _toolLocationLabel(activity, l10n),
                    palette: palette,
                  ),
              _ToolActivityTechnicalDetails(
                activity: activity,
                palette: palette,
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _toolActivityName(String name, AppLocalizations l10n) =>
      switch (name) {
        'list_files' => l10n.toolListFiles,
        'search_files' => l10n.toolSearchFiles,
        'read_file' => l10n.toolReadFile,
        'get_file_info' => l10n.toolGetFileInfo,
        _ => name,
      };
}

class _ToolFileListing {
  const _ToolFileListing({
    required this.entries,
    required this.hasMore,
    required this.isIncomplete,
  });

  final List<_ToolFileEntry> entries;
  final bool hasMore;
  final bool isIncomplete;

  static _ToolFileListing? fromOutput(Object? output) {
    final value = _toolObjectMap(output);
    final rawEntries = value?['entries'];
    if (value == null || rawEntries is! List) return null;

    final entries = <_ToolFileEntry>[];
    for (final rawEntry in rawEntries) {
      final entry = _toolObjectMap(rawEntry);
      final path = entry?['path'];
      final type = entry?['type'];
      if (entry == null ||
          path is! String ||
          path.isEmpty ||
          (type != 'file' && type != 'directory')) {
        return null;
      }
      entries.add(_ToolFileEntry(path: path, isDirectory: type == 'directory'));
    }

    return _ToolFileListing(
      entries: entries,
      hasMore: value['nextOffset'] is int,
      isIncomplete: value['truncated'] == true,
    );
  }
}

class _ToolFileEntry {
  const _ToolFileEntry({required this.path, required this.isDirectory});

  final String path;
  final bool isDirectory;

  String get name {
    final segments = path.replaceAll('\\', '/').split('/');
    return segments.isEmpty ? path : segments.last;
  }
}

class _ToolFileListingResult extends StatelessWidget {
  const _ToolFileListingResult({
    required this.listing,
    required this.locationLabel,
    required this.palette,
  });

  final _ToolFileListing listing;
  final String? locationLabel;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              if (locationLabel case final label?) ...[
                Icon(
                  Icons.folder_open_rounded,
                  size: 15,
                  color: palette.secondaryIcon,
                ),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 11,
                      height: 16 / 11,
                    ),
                  ),
                ),
              ] else
                const Spacer(),
              Text(
                l10n.toolFileCount(listing.entries.length),
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  height: 16 / 11,
                ),
              ),
            ],
          ),
          const SizedBox(height: 7),
          if (listing.entries.isEmpty)
            _ToolActivityNotice(
              message: l10n.toolEmptyListing,
              palette: palette,
            )
          else
            Container(
              decoration: BoxDecoration(
                color: palette.surface,
                borderRadius: BorderRadius.circular(10),
                border: Border.all(color: palette.border),
              ),
              constraints: const BoxConstraints(maxHeight: 216),
              child: ListView.separated(
                shrinkWrap: true,
                padding: const EdgeInsets.symmetric(vertical: 4),
                itemCount: listing.entries.length,
                separatorBuilder: (context, index) => Divider(
                  height: 1,
                  indent: 32,
                  endIndent: 10,
                  color: palette.border.withValues(alpha: 0.65),
                ),
                itemBuilder: (context, index) {
                  final entry = listing.entries[index];
                  return SizedBox(
                    height: 32,
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 10),
                      child: Row(
                        children: [
                          Icon(
                            entry.isDirectory
                                ? Icons.folder_rounded
                                : Icons.insert_drive_file_outlined,
                            size: 16,
                            color: entry.isDirectory
                                ? palette.accent
                                : palette.secondaryIcon,
                          ),
                          const SizedBox(width: 8),
                          Expanded(
                            child: Tooltip(
                              message: entry.name,
                              child: Text(
                                entry.name,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  color: palette.text,
                                  fontSize: 12,
                                  height: 18 / 12,
                                ),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          if (listing.hasMore || listing.isIncomplete) ...[
            const SizedBox(height: 7),
            if (listing.hasMore)
              _ToolListingFootnote(
                message: l10n.toolMoreFilesAvailable,
                palette: palette,
              ),
            if (listing.isIncomplete)
              _ToolListingFootnote(
                message: l10n.toolListingIncomplete,
                palette: palette,
              ),
          ],
        ],
      ),
    );
  }
}

class _ToolListingFootnote extends StatelessWidget {
  const _ToolListingFootnote({required this.message, required this.palette});

  final String message;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 3),
    child: Row(
      children: [
        Icon(
          Icons.info_outline_rounded,
          size: 14,
          color: palette.secondaryIcon,
        ),
        const SizedBox(width: 6),
        Expanded(
          child: Text(
            message,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 11,
              height: 16 / 11,
            ),
          ),
        ),
      ],
    ),
  );
}

class _ToolActivityNotice extends StatelessWidget {
  const _ToolActivityNotice({required this.message, required this.palette});

  final String message;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
    decoration: BoxDecoration(
      color: palette.surface,
      borderRadius: BorderRadius.circular(10),
    ),
    child: Text(
      message,
      style: TextStyle(
        color: palette.secondaryText,
        fontSize: 12,
        height: 18 / 12,
      ),
    ),
  );
}

class _ToolActivityTechnicalDetails extends StatelessWidget {
  const _ToolActivityTechnicalDetails({
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;

    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        key: ValueKey<String>('tool-details-${activity.callId}'),
        tilePadding: const EdgeInsets.symmetric(horizontal: 2),
        childrenPadding: const EdgeInsets.fromLTRB(4, 0, 4, 8),
        visualDensity: VisualDensity.compact,
        iconColor: palette.secondaryIcon,
        collapsedIconColor: palette.secondaryIcon,
        title: Text(
          l10n.toolTechnicalDetails,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 11,
            fontWeight: FontWeight.w500,
            height: 16 / 11,
          ),
        ),
        leading: Icon(
          Icons.tune_rounded,
          size: 15,
          color: palette.secondaryIcon,
        ),
        children: [
          if (activity.targetPath case final targetPath?) ...[
            _ToolActivityValue(
              label: l10n.toolPermissionTarget,
              value: targetPath,
              palette: palette,
            ),
            const SizedBox(height: 8),
          ],
          _ToolActivityValue(
            label: l10n.toolInput,
            value: activity.arguments,
            palette: palette,
          ),
          if (activity.output != null) ...[
            const SizedBox(height: 8),
            _ToolActivityValue(
              label: l10n.toolOutput,
              value: activity.output,
              palette: palette,
            ),
          ],
        ],
      ),
    );
  }
}

Map<String, Object?>? _toolObjectMap(Object? value) {
  if (value is! Map) return null;
  final result = <String, Object?>{};
  for (final key in value.keys) {
    if (key is! String) return null;
    result[key] = value[key];
  }
  return result;
}

String? _toolLocationLabel(ChatToolActivity activity, AppLocalizations l10n) {
  final arguments = _toolObjectMap(activity.arguments);
  final path = arguments?['path'];
  if (path is! String) return null;
  final normalized = path.trim().toLowerCase();
  if (normalized == 'desktop:/' || normalized == 'desktop:') {
    return l10n.toolDesktopLocation;
  }
  if (normalized == 'project:/' || normalized == 'project:') {
    return l10n.toolProjectLocation;
  }
  if (normalized == 'zihora:/' || normalized == 'zihora:') {
    return l10n.toolZihoraLocation;
  }
  return null;
}

class _ToolActivityValue extends StatelessWidget {
  const _ToolActivityValue({
    required this.label,
    required this.value,
    required this.palette,
  });

  final String label;
  final Object? value;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final formattedValue = JsonEncoder.withIndent('  ').convert(value);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 4),
          child: Text(
            label,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 11,
              fontWeight: FontWeight.w500,
              height: 16 / 11,
            ),
          ),
        ),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: palette.composer,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: palette.border),
          ),
          child: SelectableText(
            formattedValue,
            style: TextStyle(
              color: palette.text,
              fontFamily: 'monospace',
              fontSize: 12,
              height: 18 / 12,
            ),
          ),
        ),
      ],
    );
  }
}

class _ReasoningSummaryAccordion extends StatelessWidget {
  const _ReasoningSummaryAccordion({
    required this.summary,
    required this.palette,
  });

  final ChatReasoningSummary summary;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final duration = summary.elapsed;
    final title = duration == null
        ? l10n.reasoningSummary
        : l10n.reasoningSummaryWithDuration(_formatDuration(duration, l10n));

    return Tooltip(
      message: l10n.reasoningSummaryTooltip,
      child: Theme(
        data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          tilePadding: const EdgeInsets.symmetric(horizontal: 10),
          childrenPadding: const EdgeInsets.fromLTRB(36, 0, 10, 8),
          visualDensity: VisualDensity.compact,
          iconColor: palette.secondaryIcon,
          collapsedIconColor: palette.secondaryIcon,
          leading: Icon(
            Icons.psychology_alt_outlined,
            color: palette.secondaryIcon,
            size: 17,
          ),
          title: Text(
            title,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 12,
              fontWeight: FontWeight.w500,
              height: 18 / 12,
            ),
          ),
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: _AssistantResponseContent(
                content: summary.content,
                isStreaming: !summary.isComplete,
                palette: palette,
              ),
            ),
          ],
        ),
      ),
    );
  }

  String _formatDuration(Duration duration, AppLocalizations l10n) {
    final seconds = duration.inSeconds;
    if (seconds < 60) return l10n.secondsShort(seconds);

    final minutes = seconds ~/ 60;
    final remainingSeconds = (seconds % 60).toString().padLeft(2, '0');
    return '$minutes:$remainingSeconds';
  }
}

class _AssistantResponseContent extends StatelessWidget {
  const _AssistantResponseContent({
    required this.content,
    required this.isStreaming,
    required this.palette,
  });

  final String content;
  final bool isStreaming;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final textStyle = TextStyle(
      color: palette.text,
      fontSize: 14,
      fontWeight: FontWeight.w400,
      height: 22 / 14,
    );
    if (isStreaming) return Text(content, style: textStyle);

    final markdownHtml = markdown.markdownToHtml(
      content,
      extensionSet: markdown.ExtensionSet.gitHubFlavored,
      encodeHtml: false,
    );
    final safeHtml = _sanitizeAssistantHtml(markdownHtml);

    return HtmlWidget(
      safeHtml,
      enableCaching: true,
      renderMode: RenderMode.column,
      textStyle: textStyle,
      onTapUrl: (_) => true,
      customStylesBuilder: (element) => switch (element.localName) {
        'pre' => {
          'background-color': _cssColor(palette.composer),
          'padding': '12px',
          'white-space': 'pre-wrap',
        },
        'code' => {
          'background-color': _cssColor(palette.composer),
          'font-family': 'monospace',
        },
        'blockquote' => {
          'border-left': '2px solid ${_cssColor(palette.border)}',
          'padding-left': '12px',
          'color': _cssColor(palette.secondaryText),
        },
        _ => null,
      },
    );
  }
}

String _sanitizeAssistantHtml(String source) {
  final fragment = html_parser.parseFragment(source);
  const blockedTags = <String>{
    'script',
    'style',
    'iframe',
    'object',
    'embed',
    'form',
    'input',
    'textarea',
    'select',
    'option',
    'button',
    'img',
    'video',
    'audio',
    'source',
    'link',
    'meta',
    'base',
    'svg',
    'canvas',
  };
  for (final element in fragment.querySelectorAll('*').toList()) {
    if (blockedTags.contains(element.localName)) {
      element.remove();
      continue;
    }
    element.attributes.removeWhere(
      (name, _) =>
          !const {'class', 'colspan', 'rowspan', 'start'}.contains(name),
    );
  }
  return fragment.outerHtml;
}

String _cssColor(Color color) {
  final rgb = color.toARGB32() & 0x00ffffff;
  return '#${rgb.toRadixString(16).padLeft(6, '0')}';
}

class _CopyMessageButton extends StatelessWidget {
  const _CopyMessageButton({required this.content});

  final String content;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: context.zihoraL10n.copyMessage,
      visualDensity: VisualDensity.compact,
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints.tightFor(width: 44, height: 44),
      onPressed: () => unawaited(_copyMessage(context, content)),
      icon: const Icon(Icons.copy_rounded, size: 16),
    );
  }
}

Future<void> _copyMessage(BuildContext context, String content) async {
  final l10n = context.zihoraL10n;
  try {
    await Clipboard.setData(ClipboardData(text: content));
  } on PlatformException {
    if (context.mounted) {
      showZihoraToast(
        context,
        l10n.messageCopyFailed,
        type: ZihoraToastType.error,
      );
    }
    return;
  } on MissingPluginException {
    if (context.mounted) {
      showZihoraToast(
        context,
        l10n.messageCopyFailed,
        type: ZihoraToastType.error,
      );
    }
    return;
  }

  if (context.mounted) {
    showZihoraToast(context, l10n.messageCopied, type: ZihoraToastType.success);
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
    final palette = ZihoraPalette.of(context);

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
      showZihoraToast(
        context,
        context.zihoraL10n.conversationTitleRequired,
        type: ZihoraToastType.error,
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
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);
    final canRename =
        widget.conversationId != null && widget.onRenameConversation != null;

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onPanStart: ZihoraWindowControls.isSupported
          ? (_) => unawaited(ZihoraWindowControls.startDragging())
          : null,
      child: SizedBox(
        height: 68,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: ZihoraSpacing.pageHorizontal,
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
