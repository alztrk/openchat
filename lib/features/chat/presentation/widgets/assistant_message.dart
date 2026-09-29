import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:html/parser.dart' as html_parser;
import 'package:intl/intl.dart';
import 'package:markdown/markdown.dart' as markdown;

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_activity.dart';

class AssistantMessage extends StatelessWidget {
  const AssistantMessage({
    super.key,
    required this.message,
    required this.modelLabel,
    required this.providerId,
    this.onRetry,
  });

  final ChatMessage message;
  final String? modelLabel;
  final String providerId;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final conversationStyle = OpenChatConversationStyle.of(context);
    final l10n = context.openchatL10n;
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
        constraints: BoxConstraints(
          maxWidth:
              conversationStyle.maxWidth *
              (780 / OpenChatSpacing.conversationMaxWidth),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _AssistantModelHeader(
              modelLabel: modelLabel,
              providerId: providerId,
            ),
            for (final summary in message.reasoningSummaries.where(
              (summary) => summary.content.trim().isNotEmpty,
            )) ...[
              const SizedBox(height: 8),
              _ReasoningSummaryAccordion(summary: summary, palette: palette),
            ],
            for (final activity in message.toolActivities) ...[
              const SizedBox(height: 8),
              ToolActivityAccordion(activity: activity, palette: palette),
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
                  CopyMessageButton(content: message.content),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }

  String? _responseStatusLabel(BuildContext context, ChatMessage message) {
    final l10n = context.openchatL10n;

    return switch (message.status) {
      ChatMessageStatus.streaming => null,
      ChatMessageStatus.completed => null,
      ChatMessageStatus.failed => l10n.responseFailed,
      ChatMessageStatus.stopped => l10n.responseStopped,
    };
  }
}

class _AssistantModelHeader extends StatelessWidget {
  const _AssistantModelHeader({
    required this.modelLabel,
    required this.providerId,
  });

  final String? modelLabel;
  final String providerId;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final displayModelLabel = modelLabel?.trim();

    return SizedBox(
      height: 20,
      child: Row(
        children: [
          ProviderIcon(
            providerId: providerId,
            color: palette.secondaryIcon,
            size: 18,
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

class AssistantMessageSkeleton extends StatelessWidget {
  const AssistantMessageSkeleton({
    required this.modelLabel,
    required this.providerId,
    super.key,
  });

  final String? modelLabel;
  final String providerId;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final conversationStyle = OpenChatConversationStyle.of(context);

    return Align(
      alignment: Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: BoxConstraints(
          maxWidth:
              conversationStyle.maxWidth *
              (780 / OpenChatSpacing.conversationMaxWidth),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _AssistantModelHeader(
              modelLabel: modelLabel,
              providerId: providerId,
            ),
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

  final OpenChatPalette palette;

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
      label: context.openchatL10n.responseInProgress,
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

  Widget _skeletonLine(double width, OpenChatPalette palette) {
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

class _ReasoningSummaryAccordion extends StatelessWidget {
  const _ReasoningSummaryAccordion({
    required this.summary,
    required this.palette,
  });

  final ChatReasoningSummary summary;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
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

class _AssistantResponseContent extends StatefulWidget {
  const _AssistantResponseContent({
    required this.content,
    required this.isStreaming,
    required this.palette,
  });

  final String content;
  final bool isStreaming;
  final OpenChatPalette palette;

  @override
  State<_AssistantResponseContent> createState() =>
      _AssistantResponseContentState();
}

class _AssistantResponseContentState extends State<_AssistantResponseContent> {
  String? _cachedContent;
  String? _cachedSafeHtml;

  @override
  void didUpdateWidget(covariant _AssistantResponseContent oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.content != widget.content) {
      _cachedContent = null;
      _cachedSafeHtml = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final conversationStyle = OpenChatConversationStyle.of(context);
    final textStyle = TextStyle(
      color: widget.palette.text,
      fontFamily: conversationStyle.fontFamily,
      fontSize: 14,
      fontWeight: FontWeight.w400,
      height: 22 / 14,
    );
    if (widget.isStreaming) return Text(widget.content, style: textStyle);

    final cachedHtml = _cachedSafeHtml;
    if (_cachedContent == widget.content && cachedHtml != null) {
      return _buildHtmlWidget(cachedHtml, textStyle);
    }
    final markdownHtml = markdown.markdownToHtml(
      widget.content,
      extensionSet: markdown.ExtensionSet.gitHubFlavored,
      encodeHtml: false,
    );
    final safeHtml = _sanitizeAssistantHtml(markdownHtml);
    _cachedContent = widget.content;
    _cachedSafeHtml = safeHtml;

    return _buildHtmlWidget(safeHtml, textStyle);
  }

  Widget _buildHtmlWidget(String safeHtml, TextStyle textStyle) {
    return HtmlWidget(
      safeHtml,
      enableCaching: true,
      renderMode: RenderMode.column,
      textStyle: textStyle,
      onTapUrl: (_) => true,
      customStylesBuilder: (element) => switch (element.localName) {
        'pre' => {
          'background-color': _cssColor(widget.palette.composer),
          'padding': '12px',
          'white-space': 'pre-wrap',
        },
        'code' => {
          'background-color': _cssColor(widget.palette.composer),
          'font-family': 'monospace',
        },
        'blockquote' => {
          'border-left': '2px solid ${_cssColor(widget.palette.border)}',
          'padding-left': '12px',
          'color': _cssColor(widget.palette.secondaryText),
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

class CopyMessageButton extends StatelessWidget {
  const CopyMessageButton({required this.content, super.key});

  final String content;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: context.openchatL10n.copyMessage,
      visualDensity: VisualDensity.compact,
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints.tightFor(width: 44, height: 44),
      onPressed: () => unawaited(_copyMessage(context, content)),
      icon: const Icon(Icons.copy_rounded, size: 16),
    );
  }
}

Future<void> _copyMessage(BuildContext context, String content) async {
  final l10n = context.openchatL10n;
  try {
    await Clipboard.setData(ClipboardData(text: content));
  } on PlatformException {
    if (context.mounted) {
      showOpenChatToast(
        context,
        l10n.messageCopyFailed,
        type: OpenChatToastType.error,
      );
    }
    return;
  } on MissingPluginException {
    if (context.mounted) {
      showOpenChatToast(
        context,
        l10n.messageCopyFailed,
        type: OpenChatToastType.error,
      );
    }
    return;
  }

  if (context.mounted) {
    showOpenChatToast(
      context,
      l10n.messageCopied,
      type: OpenChatToastType.success,
    );
  }
}
