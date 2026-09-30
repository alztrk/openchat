import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_web_search_view.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

Widget? toolOperationResult(
  ChatToolActivity activity,
  OpenChatPalette palette,
) => switch (activity.name) {
  'search_files' ||
  'grep' => ToolSearchResult(activity: activity, palette: palette),
  'read_file' || 'read' => ToolReadResult(activity: activity, palette: palette),
  'get_file_info' => ToolFileInfoResult(activity: activity, palette: palette),
  'write_file' ||
  'write' => ToolWriteResult(activity: activity, palette: palette),
  'edit_file' || 'edit' => ToolEditResult(activity: activity, palette: palette),
  'web_search' => ToolWebSearchResult(activity: activity, palette: palette),
  'read_url_content' ||
  'read_url' => ToolReadUrlResult(activity: activity, palette: palette),
  _ => null,
};

IconData toolOperationIcon(String name) => switch (name) {
  'list_files' || 'glob' || 'list_directory' => Icons.folder_copy_outlined,
  'search_files' || 'grep' => Icons.search_rounded,
  'read_file' || 'read' => Icons.article_outlined,
  'get_file_info' => Icons.info_outline_rounded,
  'write_file' || 'write' => Icons.save_outlined,
  'edit_file' || 'edit' => Icons.edit_note_rounded,
  'execute_command' || 'bash' => Icons.terminal_rounded,
  'send_terminal_input' => Icons.keyboard_alt_outlined,
  'web_search' => Icons.travel_explore_rounded,
  'read_url_content' || 'read_url' => Icons.public_rounded,
  _ => Icons.build_outlined,
};

class ToolSearchResult extends StatelessWidget {
  const ToolSearchResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final path = _requestedPath(activity, arguments);
    final state = _activityState(context, activity, palette, path);
    if (state != null) return state;

    final output = toolActivityObjectMap(activity.output);
    final rawMatches = output?['matches'];
    final matches = <_SearchMatch>[];
    if (rawMatches is List) {
      for (final rawMatch in rawMatches) {
        final match = toolActivityObjectMap(rawMatch);
        final matchPath = match?['path'];
        final line = match?['line'];
        final text = match?['text'];
        if (match == null ||
            matchPath is! String ||
            matchPath.isEmpty ||
            line is! int ||
            line < 1 ||
            text is! String) {
          return _ToolResultMessage(
            message: context.openchatL10n.toolOperationUnavailable,
            palette: palette,
          );
        }
        matches.add(_SearchMatch(path: matchPath, line: line, text: text));
      }
    } else {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final l10n = context.openchatL10n;
    final query = arguments?['query'];
    final truncated = output?['truncated'] == true;
    final hasMore = output?['nextOffset'] is int;

    return _ResultColumn(
      palette: palette,
      children: [
        Row(
          children: [
            Icon(Icons.search_rounded, size: 15, color: palette.accentIcon),
            const SizedBox(width: 7),
            if (query is String && query.isNotEmpty)
              Expanded(
                child: Text(
                  query,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              )
            else
              const Spacer(),
            const SizedBox(width: 8),
            _CountBadge(
              label: l10n.toolSearchMatchCount(matches.length),
              palette: palette,
            ),
          ],
        ),
        if (matches.isEmpty) ...[
          const SizedBox(height: 8),
          _ToolResultMessage(
            icon: Icons.search_off_rounded,
            message: l10n.toolSearchNoMatches,
            palette: palette,
          ),
        ] else ...[
          const SizedBox(height: 8),
          Container(
            constraints: const BoxConstraints(maxHeight: 250),
            decoration: _resultDecoration(palette),
            child: ListView.separated(
              shrinkWrap: true,
              padding: const EdgeInsets.symmetric(vertical: 3),
              itemCount: matches.length,
              separatorBuilder: (context, index) => Divider(
                height: 1,
                indent: 12,
                endIndent: 12,
                color: palette.border.withValues(alpha: 0.65),
              ),
              itemBuilder: (context, index) {
                final match = matches[index];
                return Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 8,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Icon(
                            Icons.insert_drive_file_outlined,
                            size: 14,
                            color: palette.secondaryIcon,
                          ),
                          const SizedBox(width: 6),
                          Expanded(
                            child: SelectableText(
                              match.path,
                              maxLines: 1,
                              style: TextStyle(
                                color: palette.secondaryText,
                                fontSize: 11,
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          _LineBadge(line: match.line, palette: palette),
                        ],
                      ),
                      const SizedBox(height: 5),
                      Padding(
                        padding: const EdgeInsets.only(left: 20),
                        child: SelectableText(
                          match.text,
                          style: TextStyle(
                            color: palette.text,
                            fontFamily: 'monospace',
                            fontSize: 11,
                            height: 16 / 11,
                          ),
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ],
        if (hasMore || (truncated && !hasMore)) ...[
          const SizedBox(height: 7),
          _ResultFootnote(
            message: hasMore
                ? l10n.toolSearchMoreResults
                : l10n.toolOperationTruncated,
            palette: palette,
          ),
        ],
      ],
    );
  }
}

class ToolReadResult extends StatelessWidget {
  const ToolReadResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final path = _requestedPath(activity, arguments);
    final state = _activityState(context, activity, palette, path);
    if (state != null) return state;

    final output = toolActivityObjectMap(activity.output);
    final rawLines = output?['lines'];
    final lines = <_ReadLine>[];
    if (rawLines is List) {
      for (final rawLine in rawLines) {
        final line = toolActivityObjectMap(rawLine);
        final number = line?['line'];
        final text = line?['text'];
        if (line == null || number is! int || number < 1 || text is! String) {
          return _ToolResultMessage(
            message: context.openchatL10n.toolOperationUnavailable,
            palette: palette,
          );
        }
        lines.add(_ReadLine(number: number, text: text));
      }
    } else {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final resultPath = output?['path'];
    if (resultPath is! String || resultPath.isEmpty) {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final l10n = context.openchatL10n;
    final hasMore = output?['nextLine'] is int;

    return _ResultColumn(
      palette: palette,
      children: [
        _PathHeading(
          path: resultPath,
          icon: Icons.article_outlined,
          palette: palette,
        ),
        const SizedBox(height: 7),
        if (lines.isEmpty)
          _ToolResultMessage(
            icon: Icons.article_outlined,
            message: l10n.toolReadNoLines,
            palette: palette,
          )
        else
          Container(
            constraints: const BoxConstraints(maxHeight: 250),
            decoration: _resultDecoration(palette),
            child: ListView.separated(
              shrinkWrap: true,
              padding: const EdgeInsets.symmetric(vertical: 6),
              itemCount: lines.length,
              separatorBuilder: (context, index) => Divider(
                height: 1,
                indent: 48,
                endIndent: 8,
                color: palette.border.withValues(alpha: 0.45),
              ),
              itemBuilder: (context, index) {
                final line = lines[index];
                return Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 3,
                  ),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: 35,
                        child: Text(
                          '${line.number}',
                          textAlign: TextAlign.right,
                          style: TextStyle(
                            color: palette.secondaryIcon,
                            fontFamily: 'monospace',
                            fontSize: 10,
                            height: 18 / 10,
                          ),
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: SelectableText(
                          line.text.isEmpty ? ' ' : line.text,
                          style: TextStyle(
                            color: palette.text,
                            fontFamily: 'monospace',
                            fontSize: 11,
                            height: 18 / 11,
                          ),
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        const SizedBox(height: 6),
        _CountBadge(
          label: l10n.toolReadLineCount(lines.length),
          palette: palette,
        ),
        if (hasMore) ...[
          const SizedBox(height: 6),
          _ResultFootnote(message: l10n.toolReadMoreLines, palette: palette),
        ],
      ],
    );
  }
}

class ToolFileInfoResult extends StatelessWidget {
  const ToolFileInfoResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final path = _requestedPath(activity, arguments);
    final state = _activityState(context, activity, palette, path);
    if (state != null) return state;

    final output = toolActivityObjectMap(activity.output);
    final resultPath = output?['path'];
    final type = output?['type'];
    final size = output?['size'];
    if (resultPath is! String ||
        resultPath.isEmpty ||
        (type != 'file' && type != 'directory') ||
        size is! int ||
        size < 0) {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final l10n = context.openchatL10n;
    final isDirectory = type == 'directory';
    return _ResultColumn(
      palette: palette,
      children: [
        Container(
          padding: const EdgeInsets.all(12),
          decoration: _resultDecoration(palette),
          child: Row(
            children: [
              Container(
                width: 38,
                height: 38,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: palette.selected,
                  borderRadius: BorderRadius.circular(11),
                ),
                child: Icon(
                  isDirectory
                      ? Icons.folder_rounded
                      : Icons.insert_drive_file_outlined,
                  size: 20,
                  color: palette.accentIcon,
                ),
              ),
              const SizedBox(width: 11),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SelectableText(
                      _pathName(resultPath),
                      maxLines: 1,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 2),
                    SelectableText(
                      resultPath,
                      maxLines: 2,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 11,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 7),
        Wrap(
          spacing: 7,
          runSpacing: 7,
          children: [
            _MetadataChip(
              icon: isDirectory
                  ? Icons.folder_outlined
                  : Icons.description_outlined,
              label: isDirectory
                  ? l10n.toolFileTypeDirectory
                  : l10n.toolFileTypeFile,
              palette: palette,
            ),
            if (!isDirectory)
              _MetadataChip(
                icon: Icons.data_usage_outlined,
                label: _formatBytes(context, size),
                palette: palette,
              ),
          ],
        ),
      ],
    );
  }
}

class ToolWriteResult extends StatelessWidget {
  const ToolWriteResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final path = _requestedPath(activity, arguments);
    final state = _activityState(context, activity, palette, path);
    if (state != null) return state;

    final output = toolActivityObjectMap(activity.output);
    final resultPath = output?['path'];
    final bytesWritten = output?['bytesWritten'];
    if (output?['success'] != true ||
        resultPath is! String ||
        resultPath.isEmpty ||
        bytesWritten is! int ||
        bytesWritten < 0) {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final content = arguments?['content'];
    final preview = content is String ? _preview(content) : null;
    return _ResultColumn(
      palette: palette,
      children: [
        _SuccessPanel(
          icon: Icons.save_rounded,
          title: context.openchatL10n.toolWriteSuccess(
            _formatBytes(context, bytesWritten),
          ),
          palette: palette,
        ),
        const SizedBox(height: 7),
        _PathHeading(
          path: resultPath,
          icon: Icons.insert_drive_file_outlined,
          palette: palette,
        ),
        if (preview != null) ...[
          const SizedBox(height: 7),
          _PreviewBlock(
            label: context.openchatL10n.toolFilePreview,
            value: preview,
            palette: palette,
          ),
        ],
      ],
    );
  }
}

class ToolEditResult extends StatelessWidget {
  const ToolEditResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final path = _requestedPath(activity, arguments);
    final state = _activityState(context, activity, palette, path);
    if (state != null) return state;

    final output = toolActivityObjectMap(activity.output);
    final resultPath = output?['path'];
    final replacements = output?['replacements'];
    if (output?['success'] != true ||
        resultPath is! String ||
        resultPath.isEmpty ||
        replacements is! int ||
        replacements < 1) {
      return _ToolResultMessage(
        message: context.openchatL10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final oldString = arguments?['oldString'] ?? arguments?['old_string'];
    final newString = arguments?['newString'] ?? arguments?['new_string'];
    final canPreview = oldString is String && newString is String;
    return _ResultColumn(
      palette: palette,
      children: [
        _SuccessPanel(
          icon: Icons.edit_rounded,
          title: context.openchatL10n.toolEditSuccess(replacements),
          palette: palette,
        ),
        const SizedBox(height: 7),
        _PathHeading(
          path: resultPath,
          icon: Icons.insert_drive_file_outlined,
          palette: palette,
        ),
        if (canPreview) ...[
          const SizedBox(height: 7),
          LayoutBuilder(
            builder: (context, constraints) {
              final before = _PreviewBlock(
                label: context.openchatL10n.toolEditBefore,
                value: _preview(oldString),
                palette: palette,
                tint: Theme.of(context).colorScheme.error,
              );
              final after = _PreviewBlock(
                label: context.openchatL10n.toolEditAfter,
                value: _preview(newString),
                palette: palette,
                tint: palette.accent,
              );
              if (constraints.maxWidth < 500) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [before, const SizedBox(height: 6), after],
                );
              }
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: before),
                  const SizedBox(width: 7),
                  Expanded(child: after),
                ],
              );
            },
          ),
        ],
      ],
    );
  }
}

class _SearchMatch {
  const _SearchMatch({
    required this.path,
    required this.line,
    required this.text,
  });

  final String path;
  final int line;
  final String text;
}

class _ReadLine {
  const _ReadLine({required this.number, required this.text});

  final int number;
  final String text;
}

class _ResultColumn extends StatelessWidget {
  const _ResultColumn({required this.palette, required this.children});

  final OpenChatPalette palette;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: children,
    ),
  );
}

class _ToolResultMessage extends StatelessWidget {
  const _ToolResultMessage({
    required this.message,
    required this.palette,
    this.icon = Icons.info_outline_rounded,
  });

  final String message;
  final OpenChatPalette palette;
  final IconData icon;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 9),
    decoration: _resultDecoration(palette),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 15, color: palette.secondaryIcon),
        const SizedBox(width: 7),
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

class _ToolResultState extends StatelessWidget {
  const _ToolResultState({
    required this.message,
    required this.palette,
    this.path,
    this.isError = false,
    this.isWorking = false,
  });

  final String message;
  final String? path;
  final bool isError;
  final bool isWorking;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final errorColor = Theme.of(context).colorScheme.error;
    final foreground = isError ? errorColor : palette.secondaryIcon;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: isError ? errorColor.withValues(alpha: 0.08) : palette.surface,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(
            color: isError
                ? errorColor.withValues(alpha: 0.25)
                : palette.border,
          ),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (isWorking)
              SizedBox.square(
                dimension: 15,
                child: CircularProgressIndicator(
                  strokeWidth: 1.7,
                  color: palette.accent,
                ),
              )
            else
              Icon(
                isError
                    ? Icons.error_outline_rounded
                    : Icons.info_outline_rounded,
                size: 16,
                color: foreground,
              ),
            const SizedBox(width: 8),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    message,
                    style: TextStyle(
                      color: isError ? errorColor : palette.text,
                      fontSize: 11,
                      height: 16 / 11,
                    ),
                  ),
                  if (path case final path? when path.isNotEmpty) ...[
                    const SizedBox(height: 3),
                    SelectableText(
                      path,
                      maxLines: 2,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 10,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SuccessPanel extends StatelessWidget {
  const _SuccessPanel({
    required this.icon,
    required this.title,
    required this.palette,
  });

  final IconData icon;
  final String title;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
    decoration: BoxDecoration(
      color: palette.accent.withValues(alpha: 0.09),
      borderRadius: BorderRadius.circular(9),
    ),
    child: Row(
      children: [
        Icon(icon, size: 16, color: palette.accentIcon),
        const SizedBox(width: 7),
        Expanded(
          child: Text(
            title,
            style: TextStyle(
              color: palette.text,
              fontSize: 11,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
        Icon(Icons.check_rounded, size: 15, color: palette.accentIcon),
      ],
    ),
  );
}

class _PathHeading extends StatelessWidget {
  const _PathHeading({
    required this.path,
    required this.icon,
    required this.palette,
  });

  final String path;
  final IconData icon;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Icon(icon, size: 14, color: palette.secondaryIcon),
      const SizedBox(width: 6),
      Expanded(
        child: SelectableText(
          path,
          maxLines: 2,
          style: TextStyle(color: palette.secondaryText, fontSize: 11),
        ),
      ),
    ],
  );
}

class _PreviewBlock extends StatelessWidget {
  const _PreviewBlock({
    required this.label,
    required this.value,
    required this.palette,
    this.tint,
  });

  final String label;
  final String value;
  final OpenChatPalette palette;
  final Color? tint;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(9),
    decoration: BoxDecoration(
      color: tint == null
          ? palette.surface
          : Color.alphaBlend(tint!.withValues(alpha: 0.08), palette.surface),
      borderRadius: BorderRadius.circular(9),
      border: Border.all(color: palette.border),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 10,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 5),
        SelectableText(
          value,
          style: TextStyle(
            color: palette.text,
            fontFamily: 'monospace',
            fontSize: 10,
            height: 15 / 10,
          ),
        ),
      ],
    ),
  );
}

class _CountBadge extends StatelessWidget {
  const _CountBadge({required this.label, required this.palette});

  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
    decoration: BoxDecoration(
      color: palette.surface,
      borderRadius: BorderRadius.circular(20),
    ),
    child: Text(
      label,
      style: TextStyle(
        color: palette.secondaryText,
        fontSize: 10,
        fontWeight: FontWeight.w500,
      ),
    ),
  );
}

class _LineBadge extends StatelessWidget {
  const _LineBadge({required this.line, required this.palette});

  final int line;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
    decoration: BoxDecoration(
      color: palette.selected,
      borderRadius: BorderRadius.circular(6),
    ),
    child: Text(
      '$line',
      style: TextStyle(
        color: palette.secondaryText,
        fontFamily: 'monospace',
        fontSize: 10,
      ),
    ),
  );
}

class _MetadataChip extends StatelessWidget {
  const _MetadataChip({
    required this.icon,
    required this.label,
    required this.palette,
  });

  final IconData icon;
  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
    decoration: BoxDecoration(
      color: palette.surface,
      borderRadius: BorderRadius.circular(8),
      border: Border.all(color: palette.border),
    ),
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 13, color: palette.secondaryIcon),
        const SizedBox(width: 5),
        Text(
          label,
          style: TextStyle(color: palette.secondaryText, fontSize: 10),
        ),
      ],
    ),
  );
}

class _ResultFootnote extends StatelessWidget {
  const _ResultFootnote({required this.message, required this.palette});

  final String message;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Icon(Icons.info_outline_rounded, size: 13, color: palette.secondaryIcon),
      const SizedBox(width: 5),
      Expanded(
        child: Text(
          message,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 10,
            height: 15 / 10,
          ),
        ),
      ),
    ],
  );
}

BoxDecoration _resultDecoration(OpenChatPalette palette) => BoxDecoration(
  color: palette.surface,
  borderRadius: BorderRadius.circular(10),
  border: Border.all(color: palette.border),
);

Widget? _activityState(
  BuildContext context,
  ChatToolActivity activity,
  OpenChatPalette palette,
  String? path,
) {
  final l10n = context.openchatL10n;
  final output = toolActivityObjectMap(activity.output);
  final error = toolActivityObjectMap(output?['error']);
  final message = error?['message'];
  if (activity.status == ChatToolActivityStatus.denied) {
    return _ToolResultState(
      message: l10n.toolDenied,
      path: path,
      palette: palette,
    );
  }
  if (activity.status == ChatToolActivityStatus.cancelled) {
    return _ToolResultState(
      message: l10n.toolCancelled,
      path: path,
      palette: palette,
    );
  }
  if (activity.status == ChatToolActivityStatus.failed) {
    return _ToolResultState(
      message: message is String && message.isNotEmpty
          ? message
          : l10n.toolOperationFailed,
      path: path,
      isError: true,
      palette: palette,
    );
  }
  if (message is String && message.isNotEmpty) {
    return _ToolResultState(
      message: message,
      path: path,
      isError: true,
      palette: palette,
    );
  }

  if (activity.output == null) {
    final isRunning = activity.status == ChatToolActivityStatus.running;
    final isAwaiting =
        activity.status == ChatToolActivityStatus.awaitingApproval;
    return _ToolResultState(
      message: switch ((isRunning, isAwaiting)) {
        (true, _) => l10n.toolOperationWorking,
        (_, true) => l10n.toolAwaitingApproval,
        _ => l10n.toolOperationUnavailable,
      },
      path: path,
      isWorking: isRunning,
      palette: palette,
    );
  }
  return null;
}

String? _requestedPath(
  ChatToolActivity activity,
  Map<String, Object?>? arguments,
) {
  final argumentPath = arguments?['path'];
  if (argumentPath is String && argumentPath.isNotEmpty) return argumentPath;
  final targetPath = activity.targetPath;
  return targetPath == null || targetPath.isEmpty ? null : targetPath;
}

String _pathName(String path) {
  final parts = path.replaceAll('\\', '/').split('/');
  return parts.isEmpty || parts.last.isEmpty ? path : parts.last;
}

String _formatBytes(BuildContext context, int bytes) {
  final locale = Localizations.localeOf(context).toString();
  if (bytes < 1024) {
    return '${NumberFormat.decimalPattern(locale).format(bytes)} B';
  }
  var value = bytes.toDouble();
  const units = ['KB', 'MB', 'GB', 'TB'];
  var unitIndex = -1;
  do {
    value /= 1024;
    unitIndex++;
  } while (value >= 1024 && unitIndex < units.length - 1);
  final formatted = NumberFormat.decimalPatternDigits(
    locale: locale,
    decimalDigits: 1,
  ).format(value);
  return '$formatted ${units[unitIndex]}';
}

String _preview(String text) {
  const maxCharacters = 260;
  const maxLines = 3;
  final normalized = text.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
  final lines = normalized.split('\n');
  final shown = lines.take(maxLines).join('\n');
  if (lines.length > maxLines || normalized.length > maxCharacters) {
    final clipped = shown.length > maxCharacters
        ? shown.substring(0, maxCharacters)
        : shown;
    return '$clipped…';
  }
  return shown.isEmpty ? ' ' : shown;
}
