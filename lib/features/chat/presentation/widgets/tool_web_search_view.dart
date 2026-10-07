import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
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
        ),
      );
    }

    if (results.isEmpty) {
      return ToolActivityNotice(
        message: l10n.toolWebSearchNoResults,
        palette: palette,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
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
                    fontSize: 11,
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
                    fontSize: 10,
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

class _WebSearchResultItem {
  const _WebSearchResultItem({
    required this.title,
    required this.url,
    required this.snippet,
    required this.engine,
  });

  final String title;
  final String url;
  final String snippet;
  final String engine;
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
                    fontSize: 9,
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
                    fontSize: 10,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
              IconButton(
                tooltip: context.openchatL10n.toolCopyUrl,
                onPressed: () => _copyText(context, item.url),
                icon: const Icon(LucideIcons.copy),
                iconSize: 14,
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              ),
              const SizedBox(width: 4),
              IconButton(
                tooltip: context.openchatL10n.toolOpenUrl,
                onPressed: () => _openUrlOrCopy(context, item.url),
                icon: const Icon(LucideIcons.externalLink),
                color: palette.accentIcon,
                iconSize: 14,
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
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
                fontSize: 11,
                height: 15 / 11,
              ),
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
                    fontSize: 10,
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
                      fontSize: 10,
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
                    fontSize: 10,
                    fontFamily: 'monospace',
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                tooltip: l10n.toolCopyContent,
                onPressed: () => _copyText(context, content),
                icon: const Icon(LucideIcons.copy),
                iconSize: 14,
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              ),
              const SizedBox(width: 6),
              IconButton(
                tooltip: l10n.toolOpenUrl,
                onPressed: () => _openUrlOrCopy(context, targetUrl),
                icon: const Icon(LucideIcons.externalLink),
                color: palette.accentIcon,
                iconSize: 14,
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              ),
            ],
          ),
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
                    fontSize: 11,
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
