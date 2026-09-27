import 'dart:async';

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
import '../../../../l10n/generated/app_localizations.dart';
import '../../../../l10n/zihora_localizations.dart';
import '../../../../platform/windows/window_controls.dart';
import 'chat_composer.dart';
import 'window_control_bar.dart';

class ConversationPane extends StatelessWidget {
  const ConversationPane({
    required this.messageController,
    required this.showHistoryButton,
    required this.onOpenHistory,
    required this.onSendMessage,
    this.onStopMessage,
    this.canSendMessage = false,
    this.isSending = false,
    this.models = const <ChatGptModel>[],
    this.selectedModelId,
    this.onModelSelected,
    this.reasoningOptions = const <String>[],
    this.onReasoningSelected,
    this.messages = const <ChatMessage>[],
    this.messagesLoading = false,
    this.messagesErrorDescription,
    this.conversationTitle,
    this.conversationId,
    this.titleEditRequestId,
    this.onRenameConversation,
    this.onConversationTitleEditFinished,
    this.selectedModelLabel,
    this.assistantModelLabel,
    this.reasoningLevel,
    this.showWindowControls,
    super.key,
  });

  final TextEditingController messageController;
  final bool showHistoryButton;
  final VoidCallback onOpenHistory;
  final VoidCallback onSendMessage;
  final VoidCallback? onStopMessage;
  final bool canSendMessage;
  final bool isSending;
  final List<ChatGptModel> models;
  final String? selectedModelId;
  final ValueChanged<String>? onModelSelected;
  final List<String> reasoningOptions;
  final ValueChanged<String>? onReasoningSelected;
  final List<ChatMessage> messages;
  final bool messagesLoading;
  final String? messagesErrorDescription;
  final String? conversationTitle;
  final String? conversationId;
  final String? titleEditRequestId;
  final Future<void> Function(String conversationId, String title)?
  onRenameConversation;
  final VoidCallback? onConversationTitleEditFinished;
  final String? selectedModelLabel;
  final String? assistantModelLabel;
  final String? reasoningLevel;
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
                  : messages.isEmpty
                  ? _NewConversationEmptyState(
                      title: l10n.emptyChatWelcomeTitle,
                      description: l10n.emptyChatWelcomeBody,
                    )
                  : _ConversationHistory(
                      messages: messages,
                      assistantModelLabel: assistantModelLabel,
                    ),
            ),
            Padding(
              padding: EdgeInsets.fromLTRB(
                horizontalPadding,
                0,
                horizontalPadding,
                24,
              ),
              child: ChatComposer(
                controller: messageController,
                onSendMessage: onSendMessage,
                canSendMessage: canSendMessage,
                isSending: isSending,
                onStopMessage: onStopMessage,
                modelLabel: selectedModelLabel,
                models: models,
                selectedModelId: selectedModelId,
                onModelSelected: onModelSelected,
                reasoningLevel: reasoningLevel,
                reasoningOptions: reasoningOptions,
                onReasoningSelected: onReasoningSelected,
                showReasoningSelector: hasSelectedModel,
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
    required this.assistantModelLabel,
  });

  final List<ChatMessage> messages;
  final String? assistantModelLabel;

  @override
  Widget build(BuildContext context) {
    return SelectionArea(
      child: ListView.builder(
        padding: const EdgeInsets.fromLTRB(
          ZihoraSpacing.pageHorizontal,
          18,
          ZihoraSpacing.pageHorizontal,
          24,
        ),
        itemCount: messages.length,
        itemBuilder: (context, index) {
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
                  ),
                },
              ),
            ],
          );
        },
      ),
    );
  }

  bool _isSameDay(DateTime? left, DateTime? right) {
    if (left == null || right == null) return left == null && right == null;
    return DateUtils.isSameDay(left.toLocal(), right.toLocal());
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
  const _AssistantMessage({required this.message, required this.modelLabel});

  final ChatMessage message;
  final String? modelLabel;

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
    final displayModelLabel = modelLabel?.trim();

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 780),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
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
            ),
            for (final summary in message.reasoningSummaries.where(
              (summary) => summary.content.trim().isNotEmpty,
            )) ...[
              const SizedBox(height: 8),
              _ReasoningSummaryAccordion(summary: summary, palette: palette),
            ],
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
                _CopyMessageButton(content: message.content),
              ],
            ),
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
                  message: l10n.historyOpen,
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
