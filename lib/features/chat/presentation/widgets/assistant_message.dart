import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/safe_markdown.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_attachment_gallery.dart';
import 'package:openchat/features/chat/presentation/widgets/provider_icon.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_activity.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';

class AssistantMessage extends StatelessWidget {
  const AssistantMessage({
    super.key,
    required this.message,
    required this.modelLabel,
    required this.providerId,
    this.onRetry,
    this.responseVersionIndex,
    this.responseVersionCount,
    this.onSelectResponseVersion,
  });

  final ChatMessage message;
  final String? modelLabel;
  final String providerId;
  final VoidCallback? onRetry;
  final int? responseVersionIndex;
  final int? responseVersionCount;
  final ValueChanged<int>? onSelectResponseVersion;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final conversationStyle = OpenChatConversationStyle.of(context);
    final l10n = context.openchatL10n;
    final localeName = l10n.localeName;
    final citationSources = _citationSources(message);
    final metadata = <String>[
      if (message.tokensPerSecond case final rate?)
        l10n.responseTokenRate(NumberFormat('0.#', localeName).format(rate)),
      if (message.outputTokens case final count?)
        l10n.responseTokenCount(
          NumberFormat.decimalPattern(localeName).format(count),
        ),
      if (message.createdAt case final createdAt?)
        DateFormat.Hm(localeName).format(createdAt.toLocal()),
    ];
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
              (680 / OpenChatSpacing.conversationMaxWidth),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _AssistantModelHeader(
              modelLabel: modelLabel,
              providerId: providerId,
            ),
            if (responseVersionIndex case final versionIndex?)
              if (responseVersionCount case final versionCount?)
                if (versionCount > 1 && onSelectResponseVersion != null) ...[
                  const SizedBox(height: 4),
                  _ResponseVersionSelector(
                    index: versionIndex,
                    count: versionCount,
                    onSelected: onSelectResponseVersion!,
                  ),
                ],
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
              if (message.status == ChatMessageStatus.failed) ...[
                if (message.content.trim().isNotEmpty) ...[
                  const SizedBox(height: 6),
                  _AssistantResponseContent(
                    content: message.content,
                    isStreaming: false,
                    palette: palette,
                    citationSources: citationSources,
                  ),
                ],
                const SizedBox(height: 6),
                _AssistantFailureCard(
                  description: _failureDescription(l10n, message.failureCode),
                ),
              ] else ...[
                const SizedBox(height: 6),
                _AssistantResponseContent(
                  content: message.content,
                  isStreaming: message.status == ChatMessageStatus.streaming,
                  palette: palette,
                  citationSources: citationSources,
                ),
              ],
              if (message.attachments.isNotEmpty) ...[
                const SizedBox(height: 10),
                ChatAttachmentGallery(
                  attachments: message.attachments,
                  palette: palette,
                  preferredImageWidth: 280,
                  imageHeight: 200,
                ),
              ],
              const SizedBox(height: 6),
              Wrap(
                spacing: 8,
                runSpacing: 0,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  if (metadata.isNotEmpty)
                    Text(
                      metadata.join(' · '),
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
                      icon: const Icon(LucideIcons.refreshCw, size: 17),
                      color: palette.secondaryIcon,
                    ),
                  if (message.content.trim().isNotEmpty)
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
      ChatMessageStatus.failed => null,
      ChatMessageStatus.stopped => l10n.responseStopped,
    };
  }

  String _failureDescription(AppLocalizations l10n, String? code) =>
      switch (code) {
        'opencode_free_tier_restricted' => l10n.openCodeFreeTierRestricted,
        'authentication_required' => l10n.providerAuthenticationRequired,
        'rate_limited' => l10n.providerRateLimited,
        'provider_tool_request_rejected' => l10n.providerToolRequestRejected,
        'network_unavailable' => l10n.providerNetworkUnavailable,
        'model_unavailable' => l10n.selectedModelUnavailable,
        'local_engine_not_installed' ||
        'local_engine_unavailable' => l10n.localModelEngineNotReady,
        'local_model_unavailable' => l10n.localModelInvalid,
        'local_engine_start_failed' => l10n.localModelStartError,
        'local_engine_start_timeout' => l10n.localModelStartTimeout,
        'local_engine_runtime_unavailable' => l10n.localModelRuntimeUnavailable,
        'local_engine_capability_unavailable' =>
          l10n.localModelContextUnavailable,
        'local_model_inference_failed' => l10n.localModelInferenceFailed,
        'context_window_exceeded' ||
        'context_compaction_input_too_large' => l10n.contextWindowExceeded,
        'attachment_unavailable' => l10n.attachmentUnavailable,
        'model_does_not_support_images' => l10n.modelDoesNotSupportImages,
        'provider_request_failed' ||
        'invalid_provider_response' => l10n.providerRequestFailed,
        _ => l10n.chatRequestFailed,
      };
}

class _AssistantFailureCard extends StatelessWidget {
  const _AssistantFailureCard({required this.description});

  final String description;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final palette = OpenChatPalette.of(context);
    final errorColor = theme.colorScheme.error;

    return Semantics(
      liveRegion: true,
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: BoxDecoration(
          color: Color.alphaBlend(
            errorColor.withValues(alpha: 0.07),
            palette.surface,
          ),
          border: Border.all(color: errorColor.withValues(alpha: 0.42)),
          borderRadius: BorderRadius.circular(14),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.only(top: 1),
              child: Icon(LucideIcons.circleAlert, size: 18, color: errorColor),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    context.openchatL10n.responseFailed,
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: errorColor,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    description,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: palette.text,
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

class _ResponseVersionSelector extends StatelessWidget {
  const _ResponseVersionSelector({
    required this.index,
    required this.count,
    required this.onSelected,
  });

  final int index;
  final int count;
  final ValueChanged<int> onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final versionLabel = l10n.responseVersionCount(index + 1, count);

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          tooltip: l10n.previousResponseVersion,
          onPressed: index > 0 ? () => onSelected(index - 1) : null,
          visualDensity: VisualDensity.compact,
          icon: const Icon(LucideIcons.chevronLeft, size: 16),
        ),
        Semantics(
          liveRegion: true,
          label: versionLabel,
          child: Text(
            versionLabel,
            style: Theme.of(context).textTheme.bodySmall
                ?.copyWith(color: palette.secondaryText),
          ),
        ),
        IconButton(
          tooltip: l10n.nextResponseVersion,
          onPressed: index + 1 < count ? () => onSelected(index + 1) : null,
          visualDensity: VisualDensity.compact,
          icon: const Icon(LucideIcons.chevronRight, size: 16),
        ),
      ],
    );
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

    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 20),
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
                color: palette.text,
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
              (680 / OpenChatSpacing.conversationMaxWidth),
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

    final cardRadius = BorderRadius.circular(OpenChatRadii.card);

    return Tooltip(
      message: l10n.reasoningSummaryTooltip,
      child: ChatSurfaceCard(
        child: Material(
          color: Colors.transparent,
          child: Theme(
            data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
            child: ExpansionTile(
              tilePadding: const EdgeInsets.symmetric(horizontal: 10),
              childrenPadding: const EdgeInsets.fromLTRB(36, 0, 10, 8),
              visualDensity: VisualDensity.compact,
              iconColor: palette.secondaryIcon,
              collapsedIconColor: palette.secondaryIcon,
              shape: RoundedRectangleBorder(borderRadius: cardRadius),
              collapsedShape: RoundedRectangleBorder(borderRadius: cardRadius),
              leading: Icon(
                LucideIcons.brain,
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
                    citationSources: const <String, ChatCitationSource>{},
                    textColor: palette.secondaryText,
                  ),
                ),
              ],
            ),
          ),
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
    required this.citationSources,
    this.textColor,
  });

  final String content;
  final bool isStreaming;
  final OpenChatPalette palette;
  final Map<String, ChatCitationSource> citationSources;
  final Color? textColor;

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
    if (oldWidget.content != widget.content ||
        _citationSignature(oldWidget.citationSources) !=
            _citationSignature(widget.citationSources)) {
      _cachedContent = null;
      _cachedSafeHtml = null;
    }
  }

  @override
  Widget build(BuildContext context) {
    final conversationStyle = OpenChatConversationStyle.of(context);
    final textStyle = TextStyle(
      color: widget.textColor ?? widget.palette.text,
      fontFamily: conversationStyle.fontFamily,
      fontSize: 14,
      fontWeight: FontWeight.w400,
      height: 22 / 14,
    );
    if (widget.isStreaming && !_containsMarkdownSyntax(widget.content)) {
      return Text(widget.content, style: textStyle);
    }

    final cachedHtml = _cachedSafeHtml;
    if (_cachedContent == widget.content && cachedHtml != null) {
      return _buildHtmlWidget(cachedHtml, textStyle);
    }
    final linkedContent = _linkCitations(
      widget.content,
      widget.citationSources,
    );
    final safeHtml = markdownToSafeHtml(linkedContent);
    _cachedContent = widget.content;
    _cachedSafeHtml = safeHtml;

    return _buildHtmlWidget(safeHtml, textStyle);
  }

  Widget _buildHtmlWidget(String safeHtml, TextStyle textStyle) {
    return HtmlWidget(
      safeHtml,
      enableCaching: !widget.isStreaming,
      renderMode: RenderMode.column,
      textStyle: textStyle,
      onTapUrl: (url) => _openCitation(context, url, widget.citationSources),
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

Map<String, ChatCitationSource> _citationSources(ChatMessage message) {
  final sources = <String, ChatCitationSource>{
    for (final source in message.citationSources) source.id: source,
  };
  for (final activity in message.toolActivities) {
    final output = toolActivityObjectMap(activity.output);
    final rawResults = output?['results'];
    if (activity.name == 'web_search' &&
        output?['sourceType'] == 'local_web_search' &&
        rawResults is List) {
      final timestamp = _citationTime(output?['retrievedAtUnixMs']);
      for (final raw in rawResults) {
        final value = toolActivityObjectMap(raw);
        final source = _citationSource(
          id: value?['sourceId'],
          title: value?['title'],
          url: value?['url'],
          sourceType: 'local_web_search',
          snippet: value?['snippet'],
          retrievedAt: timestamp,
        );
        if (source != null) sources[source.id] = source;
      }
    } else if ((activity.name == 'read_url_content' ||
            activity.name == 'read_url') &&
        output?['sourceType'] == 'local_read_url') {
      final source = _citationSource(
        id: output?['sourceId'],
        title: output?['title'],
        url: output?['url'],
        sourceType: 'local_read_url',
        snippet: output?['content'],
        retrievedAt: _citationTime(output?['retrievedAtUnixMs']),
      );
      if (source != null) sources[source.id] = source;
    }
  }
  return sources;
}

ChatCitationSource? _citationSource({
  required Object? id,
  required Object? title,
  required Object? url,
  required String sourceType,
  required Object? snippet,
  required DateTime? retrievedAt,
}) {
  if (id is! String ||
      !RegExp(r'^[PSU]\d+(?:-[A-Za-z0-9]+)?$').hasMatch(id) ||
      title is! String ||
      url is! String) {
    return null;
  }
  final uri = Uri.tryParse(url);
  if (uri == null ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty ||
      (uri.scheme != 'https' && uri.scheme != 'http')) {
    return null;
  }
  return ChatCitationSource(
    id: id,
    title: title,
    url: uri.toString(),
    sourceType: sourceType,
    snippet: snippet is String && snippet.isNotEmpty ? snippet : null,
    retrievedAt: retrievedAt,
  );
}

DateTime? _citationTime(Object? value) {
  if (value is! int || value < 0 || value > 8640000000000000) return null;
  return DateTime.fromMillisecondsSinceEpoch(value, isUtc: true);
}

String _citationSignature(Map<String, ChatCitationSource> sources) {
  final entries =
      sources.values
          .map((source) => '${source.id}\n${source.url}\n${source.title}')
          .toList()
        ..sort();
  return entries.join('\n');
}

String _linkCitations(
  String content,
  Map<String, ChatCitationSource> sources,
) => content.replaceAllMapped(RegExp(r'\[([PSU]\d+(?:-[A-Za-z0-9]+)?)\]'), (
  match,
) {
  final id = match[1];
  if (id == null || !sources.containsKey(id)) return match[0]!;
  return '[$id](openchat-source:${Uri.encodeComponent(id)})';
});

bool _openCitation(
  BuildContext context,
  String rawUrl,
  Map<String, ChatCitationSource> sources,
) {
  final uri = Uri.tryParse(rawUrl);
  if (uri?.scheme != 'openchat-source') return true;
  final source = sources[uri!.path];
  if (source == null) return true;
  final l10n = context.openchatL10n;
  final localSourceLabel = switch (source.sourceType) {
    'local_web_search' => l10n.toolLocalWebSource,
    'local_read_url' => l10n.toolLocalPageSource,
    'provider_native' => l10n.toolProviderSource,
    _ => l10n.toolSourceDetails,
  };
  showDialog<void>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(l10n.toolSourceDetails),
      content: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(source.title, style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 8),
            Text(l10n.toolCitationSource(source.id)),
            Text(localSourceLabel),
            if (source.retrievedAt case final retrievedAt?)
              Text(
                l10n.toolSourceRetrievedAt(
                  DateFormat.yMMMd(l10n.localeName)
                      .add_jm()
                      .format(retrievedAt.toLocal()),
                ),
              ),
            const SizedBox(height: 8),
            SelectableText(source.url),
            if (source.snippet case final snippet?) ...[
              const SizedBox(height: 8),
              SelectableText(snippet),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    ),
  );
  return true;
}

final _streamingMarkdownSyntax = RegExp(
  r'(^|\n)\s{0,3}(?:#{1,6}\s|>|[-+*]\s|\d+[.)]\s|```|~~~)|'
  r'\*\*|__|~~|`|\[[^\]]+\]\(|(?:^|[^\\])\*[^*\n]+\*|'
  r'(?:^|[^\\])_[^_\n]+_|\|.+\||https?://|www\.',
  multiLine: true,
);

bool _containsMarkdownSyntax(String content) =>
    _streamingMarkdownSyntax.hasMatch(content);

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
      icon: const Icon(LucideIcons.copy, size: 16),
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
