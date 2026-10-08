import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:html/dom.dart' as html_dom;
import 'package:html/parser.dart' as html_parser;
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:intl/intl.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolWebSearchResult extends StatelessWidget {
  const ToolWebSearchResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final arguments = toolActivityObjectMap(activity.arguments);
    final output = toolActivityObjectMap(activity.output);
    final query =
        _stringField(arguments, 'query') ?? _stringField(output, 'query') ?? '';

    if (activity.output == null &&
        (activity.status == ChatToolActivityStatus.running ||
            activity.status == ChatToolActivityStatus.awaitingApproval)) {
      return ToolActivityNotice(
        message: l10n.toolOperationWorking,
        palette: palette,
        isLoading: activity.status == ChatToolActivityStatus.running,
      );
    }

    if (activity.status == ChatToolActivityStatus.denied ||
        activity.status == ChatToolActivityStatus.cancelled) {
      return ToolActivityNotice(
        message: activity.status == ChatToolActivityStatus.denied
            ? l10n.toolDenied
            : l10n.toolCancelled,
        palette: palette,
      );
    }

    if (activity.status == ChatToolActivityStatus.failed ||
        output?['error'] != null) {
      final errorMap = toolActivityObjectMap(output?['error']);
      final message =
          _stringField(errorMap, 'message') ?? l10n.toolOperationFailed;
      return ToolActivityNotice(
        message: message,
        palette: palette,
        isError: true,
      );
    }

    final rawResults = output?['results'];
    if (rawResults is! List) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }
    final sourceType = _stringField(output, 'sourceType');
    final attributionHtml = sourceType == 'provider_native'
        ? _safeGoogleAttribution(_stringField(output, 'attributionHtml'))
        : null;
    final retrievedAt = _retrievedAt(output?['retrievedAtUnixMs']);
    final results = <_WebSearchResultItem>[];
    for (final raw in rawResults) {
      final item = toolActivityObjectMap(raw);
      final title = _stringField(item, 'title');
      final url = _stringField(item, 'url');
      final snippet = _stringField(item, 'snippet');
      final engine = _stringField(item, 'engine');
      if (title == null || url == null || snippet == null || engine == null) {
        return ToolActivityNotice(
          message: l10n.toolOperationUnavailable,
          palette: palette,
        );
      }
      results.add(
        _WebSearchResultItem(
          title: title,
          url: url,
          snippet: snippet,
          engine: engine,
          sourceId: _stringField(item, 'sourceId'),
          retrievedAt: sourceType == 'local_web_search' ? retrievedAt : null,
          isLocalSource: sourceType == 'local_web_search',
        ),
      );
    }

    if (results.isEmpty) {
      if (attributionHtml != null) {
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            HtmlWidget(
              attributionHtml,
              onTapUrl: (url) async {
                await _openUrlOrCopy(context, url);
                return true;
              },
            ),
            const SizedBox(height: 8),
            ToolActivityNotice(
              message: l10n.toolWebSearchNoResults,
              palette: palette,
            ),
          ],
        );
      }
      return ToolActivityNotice(
        message: l10n.toolWebSearchNoResults,
        palette: palette,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (attributionHtml != null) ...[
          HtmlWidget(
            attributionHtml,
            onTapUrl: (url) async {
              await _openUrlOrCopy(context, url);
              return true;
            },
          ),
          const SizedBox(height: 8),
        ],
        Padding(
          padding: const EdgeInsets.only(bottom: 8),
          child: Row(
            children: [
              Icon(LucideIcons.search, size: 14, color: palette.secondaryIcon),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  query.isEmpty ? l10n.toolSearchQuery : query,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.metadata,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: palette.surface,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: palette.border),
                ),
                child: Text(
                  l10n.toolWebSearchResultCount(results.length),
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.metadata,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          ),
        ),
        for (var i = 0; i < results.length; i++) ...[
          if (i > 0) const SizedBox(height: 6),
          _WebSearchResultCard(item: results[i], palette: palette),
        ],
      ],
    );
  }
}

String? _safeGoogleAttribution(String? rawHtml) {
  if (rawHtml == null || rawHtml.isEmpty || rawHtml.length > 32 * 1024) {
    return null;
  }
  final fragment = html_parser.parseFragment(rawHtml);
  final safeRoot = html_dom.Element.tag('div');
  void copyChildren(html_dom.Node source, html_dom.Node target) {
    for (final child in source.nodes) {
      if (child is html_dom.Text) {
        target.append(html_dom.Text(child.data));
      } else if (child is html_dom.Element) {
        final tag = child.localName?.toLowerCase();
        if (tag == 'style') {
          final css = child.text;
          if (css.length <= 16 * 1024 &&
              !RegExp(
                r'@import|url\s*\(|expression\s*\(',
                caseSensitive: false,
              ).hasMatch(css)) {
            final style = html_dom.Element.tag('style')..text = css;
            target.append(style);
          }
          continue;
        }
        if (!const {
          'div',
          'span',
          'p',
          'a',
          'b',
          'strong',
          'br',
        }.contains(tag)) {
          continue;
        }
        final element = html_dom.Element.tag(tag!);
        final className = child.attributes['class'];
        if (className != null && className.length <= 256) {
          element.attributes['class'] = className;
        }
        if (tag == 'a') {
          final href = Uri.tryParse(child.attributes['href'] ?? '');
          if (href != null &&
              href.host.isNotEmpty &&
              (href.scheme == 'http' || href.scheme == 'https')) {
            element.attributes['href'] = href.toString();
          }
        }
        target.append(element);
        if (tag != 'br') copyChildren(child, element);
      }
    }
  }

  copyChildren(fragment, safeRoot);
  final sanitized = safeRoot.innerHtml;
  return sanitized.isEmpty ? null : sanitized;
}

class _WebSearchResultItem {
  const _WebSearchResultItem({
    required this.title,
    required this.url,
    required this.snippet,
    required this.engine,
    required this.sourceId,
    required this.retrievedAt,
    required this.isLocalSource,
  });

  final String title;
  final String url;
  final String snippet;
  final String engine;
  final String? sourceId;
  final DateTime? retrievedAt;
  final bool isLocalSource;
}

class _WebSearchResultCard extends StatelessWidget {
  const _WebSearchResultCard({required this.item, required this.palette});

  final _WebSearchResultItem item;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final uri = Uri.tryParse(item.url);
    final domain = uri?.host.isNotEmpty == true ? uri!.host : item.url;

    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: palette.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
                decoration: BoxDecoration(
                  color: palette.selected,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text(
                  item.engine.toUpperCase(),
                  style: TextStyle(
                    color: palette.accentIcon,
                    fontSize: OpenChatTypography.metadata,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  domain,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.metadata,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
              IconButton(
                tooltip: context.openchatL10n.toolCopyUrl,
                onPressed: () => _copyText(context, item.url),
                icon: const Icon(LucideIcons.copy),
                iconSize: 14,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(width: 44, height: 44),
              ),
              const SizedBox(width: 4),
              IconButton(
                tooltip: context.openchatL10n.toolOpenUrl,
                onPressed: () => _openUrlOrCopy(context, item.url),
                icon: const Icon(LucideIcons.externalLink),
                color: palette.accentIcon,
                iconSize: 14,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(width: 44, height: 44),
              ),
            ],
          ),
          const SizedBox(height: 5),
          InkWell(
            onTap: () => _openUrlOrCopy(context, item.url),
            child: Text(
              item.title,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: palette.text,
                fontSize: 12,
                fontWeight: FontWeight.w600,
                height: 16 / 12,
                decoration: TextDecoration.underline,
                decorationColor: palette.border,
              ),
            ),
          ),
          if (item.snippet.isNotEmpty) ...[
            const SizedBox(height: 4),
            SelectableText(
              item.snippet,
              style: TextStyle(
                color: palette.secondaryText,
                fontSize: OpenChatTypography.metadata,
                height: 15 / 11,
              ),
            ),
          ],
          if (item.isLocalSource ||
              item.sourceId != null ||
              item.retrievedAt != null) ...[
            const SizedBox(height: 6),
            Wrap(
              spacing: 7,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if (item.sourceId case final sourceId?)
                  _SourceMetadataTag(
                    label: context.openchatL10n.toolCitationSource(sourceId),
                    palette: palette,
                  ),
                if (item.isLocalSource)
                  _SourceMetadataTag(
                    label: context.openchatL10n.toolLocalWebSource,
                    palette: palette,
                  ),
                if (item.retrievedAt case final retrievedAt?)
                  Text(
                    context.openchatL10n.toolSourceRetrievedAt(
                      DateFormat.yMMMd(context.openchatL10n.localeName)
                          .add_jm()
                          .format(retrievedAt.toLocal()),
                    ),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: OpenChatTypography.metadata,
                    ),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class ToolReadUrlResult extends StatelessWidget {
  const ToolReadUrlResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final arguments = toolActivityObjectMap(activity.arguments);
    final output = toolActivityObjectMap(activity.output);

    if (activity.output == null &&
        (activity.status == ChatToolActivityStatus.running ||
            activity.status == ChatToolActivityStatus.awaitingApproval)) {
      return ToolActivityNotice(
        message: l10n.toolOperationWorking,
        palette: palette,
        isLoading: activity.status == ChatToolActivityStatus.running,
      );
    }

    if (activity.status == ChatToolActivityStatus.denied ||
        activity.status == ChatToolActivityStatus.cancelled) {
      return ToolActivityNotice(
        message: activity.status == ChatToolActivityStatus.denied
            ? l10n.toolDenied
            : l10n.toolCancelled,
        palette: palette,
      );
    }

    if (activity.status == ChatToolActivityStatus.failed ||
        output?['error'] != null) {
      final errorMap = toolActivityObjectMap(output?['error']);
      final message =
          _stringField(errorMap, 'message') ?? l10n.toolOperationFailed;
      return ToolActivityNotice(
        message: message,
        palette: palette,
        isError: true,
      );
    }

    final targetUrl =
        _stringField(output, 'url') ??
        _stringField(arguments, 'url') ??
        activity.targetPath;
    final title = _stringField(output, 'title');
    final content = _stringField(output, 'content');
    final length = output?['length'];
    final truncated = output?['truncated'];
    final sourceType = _stringField(output, 'sourceType');
    final sourceId = _stringField(output, 'sourceId');
    final retrievedAt = _retrievedAt(output?['retrievedAtUnixMs']);
    if (targetUrl == null ||
        targetUrl.isEmpty ||
        title == null ||
        content == null ||
        length is! int ||
        length < 0 ||
        truncated is! bool) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }
    final parsedUri = Uri.tryParse(targetUrl);
    final domain = parsedUri?.host.isNotEmpty == true
        ? parsedUri!.host
        : targetUrl;

    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: palette.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(LucideIcons.globe, size: 14, color: palette.accentIcon),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  title.isNotEmpty ? title : domain,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: palette.selected,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  l10n.toolReadUrlLength(length),
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.metadata,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
              if (truncated) ...[
                const SizedBox(width: 6),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: palette.selected,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    l10n.toolOperationTruncated,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                      fontSize: OpenChatTypography.metadata,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ],
          ),
          const SizedBox(height: 6),
          Row(
            children: [
              Expanded(
                child: Text(
                  targetUrl,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: OpenChatTypography.metadata,
                    fontFamily: OpenChatTypography.codeFontFamily,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                tooltip: l10n.toolCopyContent,
                onPressed: () => _copyText(context, content),
                icon: const Icon(LucideIcons.copy),
                iconSize: 14,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(width: 44, height: 44),
              ),
              const SizedBox(width: 6),
              IconButton(
                tooltip: l10n.toolOpenUrl,
                onPressed: () => _openUrlOrCopy(context, targetUrl),
                icon: const Icon(LucideIcons.externalLink),
                color: palette.accentIcon,
                iconSize: 14,
                visualDensity: VisualDensity.standard,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(width: 44, height: 44),
              ),
            ],
          ),
          if (sourceType == 'local_read_url' ||
              sourceId != null ||
              retrievedAt != null) ...[
            const SizedBox(height: 4),
            Wrap(
              spacing: 7,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if (sourceId case final id?)
                  _SourceMetadataTag(
                    label: l10n.toolCitationSource(id),
                    palette: palette,
                  ),
                if (sourceType == 'local_read_url')
                  _SourceMetadataTag(
                    label: l10n.toolLocalPageSource,
                    palette: palette,
                  ),
                if (retrievedAt case final timestamp?)
                  Text(
                    l10n.toolSourceRetrievedAt(
                      DateFormat.yMMMd(l10n.localeName)
                          .add_jm()
                          .format(timestamp.toLocal()),
                    ),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: OpenChatTypography.metadata,
                    ),
                  ),
              ],
            ),
          ],
          if (content.isNotEmpty) ...[
            const SizedBox(height: 8),
            Container(
              width: double.infinity,
              constraints: const BoxConstraints(maxHeight: 220),
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: palette.composer,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: palette.border),
              ),
              child: SingleChildScrollView(
                child: SelectableText(
                  content,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: OpenChatTypography.metadata,
                    height: 16 / 11,
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

String? _stringField(Map<String, Object?>? value, String key) {
  final field = value?[key];
  return field is String ? field : null;
}

DateTime? _retrievedAt(Object? value) {
  if (value is! int || value < 0 || value > 8640000000000000) return null;
  return DateTime.fromMillisecondsSinceEpoch(value, isUtc: true);
}

class _SourceMetadataTag extends StatelessWidget {
  const _SourceMetadataTag({required this.label, required this.palette});

  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
    decoration: BoxDecoration(
      color: palette.selected,
      borderRadius: BorderRadius.circular(6),
    ),
    child: Text(
      label,
      style: TextStyle(
        color: palette.secondaryText,
        fontSize: OpenChatTypography.metadata,
      ),
    ),
  );
}

Future<void> _copyText(BuildContext context, String value) async {
  final l10n = context.openchatL10n;
  try {
    await Clipboard.setData(ClipboardData(text: value));
  } on PlatformException {
    if (context.mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.toolCopyFailed)));
    }
    return;
  }

  if (context.mounted) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(l10n.toolTerminalCopied),
        duration: const Duration(seconds: 1),
      ),
    );
  }
}

Future<void> _openUrlOrCopy(BuildContext context, String rawUrl) async {
  final uri = Uri.tryParse(rawUrl);
  final isWebUrl =
      uri != null &&
      uri.host.isNotEmpty &&
      (uri.scheme == 'http' || uri.scheme == 'https');
  final executable = switch ((
    Platform.isWindows,
    Platform.isMacOS,
    Platform.isLinux,
  )) {
    (true, _, _) => 'explorer.exe',
    (_, true, _) => 'open',
    (_, _, true) => 'xdg-open',
    _ => null,
  };

  if (!isWebUrl || executable == null) {
    await _copyText(context, rawUrl);
    return;
  }

  try {
    await Process.start(executable, [
      uri.toString(),
    ], mode: ProcessStartMode.detached);
  } on ProcessException {
    if (!context.mounted) return;
    await _copyText(context, rawUrl);
  }
}
