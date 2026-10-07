import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/features/chat/domain/tool_permission_request.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolPermissionCard extends StatelessWidget {
  const ToolPermissionCard({
    super.key,
    required this.request,
    required this.isResponding,
    this.errorMessage,
    this.onApprove,
    this.onDeny,
  });

  final ToolPermissionRequest request;
  final bool isResponding;
  final String? errorMessage;
  final VoidCallback? onApprove;
  final VoidCallback? onDeny;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final l10n = context.openchatL10n;

    return ChatSurfaceCard(
      key: const ValueKey<String>('tool-permission-card'),
      padding: const EdgeInsets.all(12),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final details = _RequestDetails(
            request: request,
            palette: palette,
            theme: theme,
            l10n: l10n,
            errorMessage: errorMessage,
          );
          final actions = _RequestActions(
            isResponding: isResponding,
            denyLabel: l10n.toolPermissionDeny,
            approveLabel: l10n.toolPermissionAllowOnce,
            onApprove: onApprove,
            onDeny: onDeny,
          );

          if (constraints.maxWidth < 560) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _PermissionIcon(palette: palette),
                    const SizedBox(width: 10),
                    Expanded(child: details),
                  ],
                ),
                const SizedBox(height: 12),
                Align(alignment: Alignment.centerRight, child: actions),
              ],
            );
          }

          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _PermissionIcon(palette: palette),
              const SizedBox(width: 10),
              Expanded(child: details),
              const SizedBox(width: 16),
              actions,
            ],
          );
        },
      ),
    );
  }
}

class _PermissionIcon extends StatelessWidget {
  const _PermissionIcon({required this.palette});

  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    width: 34,
    height: 34,
    decoration: BoxDecoration(
      color: palette.surface,
      borderRadius: BorderRadius.circular(10),
    ),
    alignment: Alignment.center,
    child: Icon(LucideIcons.lockKeyhole, size: 18, color: palette.accentIcon),
  );
}

class _RequestDetails extends StatelessWidget {
  const _RequestDetails({
    required this.request,
    required this.palette,
    required this.theme,
    required this.l10n,
    required this.errorMessage,
  });

  final ToolPermissionRequest request;
  final OpenChatPalette palette;
  final ThemeData theme;
  final AppLocalizations l10n;
  final String? errorMessage;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(
        _toolName(request.toolName, l10n),
        style: theme.textTheme.titleSmall?.copyWith(color: palette.text),
      ),
      const SizedBox(height: 3),
      Text(
        l10n.toolPermissionRequestDescription,
        style: theme.textTheme.bodySmall?.copyWith(
          color: palette.secondaryText,
        ),
      ),
      const SizedBox(height: 7),
      Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: Icon(
              request.toolName == 'web_search' ||
                      request.toolName == 'read_url_content' ||
                      request.toolName == 'read_url'
                  ? LucideIcons.globe
                  : (request.toolName == 'execute_command' ||
                            request.toolName == 'bash' ||
                            request.toolName == 'send_terminal_input'
                        ? LucideIcons.terminal
                        : LucideIcons.folderOpen),
              size: 15,
              color: palette.secondaryIcon,
            ),
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              request.targetPath,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodySmall?.copyWith(
                color: palette.secondaryText,
              ),
            ),
          ),
        ],
      ),
      for (final detail in _requestDetails(request, l10n)) ...[
        const SizedBox(height: 8),
        _RequestArgument(
          label: detail.label,
          value: detail.value,
          palette: palette,
          theme: theme,
        ),
      ],
      if (errorMessage case final message?) ...[
        const SizedBox(height: 6),
        Text(
          message,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.error,
          ),
        ),
      ],
    ],
  );
}

class _RequestActions extends StatelessWidget {
  const _RequestActions({
    required this.isResponding,
    required this.denyLabel,
    required this.approveLabel,
    this.onApprove,
    this.onDeny,
  });

  final bool isResponding;
  final String denyLabel;
  final String approveLabel;
  final VoidCallback? onApprove;
  final VoidCallback? onDeny;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      TextButton(
        onPressed: isResponding ? null : onDeny,
        child: Text(denyLabel),
      ),
      const SizedBox(width: 8),
      FilledButton(
        onPressed: isResponding ? null : onApprove,
        child: isResponding
            ? const SizedBox.square(
                dimension: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : Text(approveLabel),
      ),
    ],
  );
}

String _toolName(String name, AppLocalizations l10n) => switch (name) {
  'list_files' || 'glob' || 'list_directory' => l10n.toolListFiles,
  'search_files' => l10n.toolSearchFiles,
  'read_file' || 'read' => l10n.toolReadFile,
  'get_file_info' => l10n.toolGetFileInfo,
  'write_file' || 'write' => l10n.toolWriteFile,
  'edit_file' || 'edit' => l10n.toolEditFile,
  'execute_command' || 'bash' => l10n.toolExecuteCommand,
  'send_terminal_input' => l10n.toolSendTerminalInput,
  'web_search' => l10n.toolWebSearch,
  'read_url_content' || 'read_url' => l10n.toolReadUrlContent,
  _ => name,
};

class _RequestArgument extends StatelessWidget {
  const _RequestArgument({
    required this.label,
    required this.value,
    required this.palette,
    required this.theme,
  });

  final String label;
  final String value;
  final OpenChatPalette palette;
  final ThemeData theme;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(
        label,
        style: theme.textTheme.labelMedium?.copyWith(
          color: palette.secondaryText,
        ),
      ),
      const SizedBox(height: 4),
      Container(
        width: double.infinity,
        constraints: const BoxConstraints(maxHeight: 88),
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: palette.surface,
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(8),
        ),
        child: SingleChildScrollView(
          child: SelectableText(
            value,
            style: theme.textTheme.bodySmall?.copyWith(color: palette.text),
          ),
        ),
      ),
    ],
  );
}

List<({String label, String value})> _requestDetails(
  ToolPermissionRequest request,
  AppLocalizations l10n,
) {
  String? argument(String key) {
    final value = request.arguments[key];
    return value is String ? value : null;
  }

  String? scalar(Object? value) => switch (value) {
    String value => value,
    num value => value.toString(),
    bool value => value ? l10n.commonYes : l10n.commonNo,
    _ => null,
  };

  ({String label, String value})? detail(String label, String? value) =>
      value == null ? null : (label: label, value: value);

  final operationDetails = switch (request.toolName) {
    'list_files' || 'glob' || 'list_directory' => [
      detail(
        l10n.toolPermissionOffset,
        scalar(request.arguments['offset'] ?? 0),
      ),
      detail(
        l10n.toolPermissionLimit,
        scalar(request.arguments['limit'] ?? 100),
      ),
    ],
    'search_files' || 'grep' => [
      detail(
        l10n.toolPermissionQuery,
        argument('query') ?? argument('pattern'),
      ),
      detail(
        l10n.toolPermissionIncludeHidden,
        scalar(request.arguments['includeHidden'] ?? false),
      ),
      detail(
        l10n.toolPermissionOffset,
        scalar(request.arguments['offset'] ?? 0),
      ),
      detail(
        l10n.toolPermissionLimit,
        scalar(request.arguments['limit'] ?? 40),
      ),
    ],
    'read_file' || 'read' => [
      detail(
        l10n.toolPermissionStartLine,
        scalar(
          request.arguments['startLine'] ?? request.arguments['offset'] ?? 1,
        ),
      ),
      detail(
        l10n.toolPermissionLineCount,
        scalar(
          request.arguments['lineCount'] ?? request.arguments['limit'] ?? 500,
        ),
      ),
    ],
    'write_file' ||
    'write' => [detail(l10n.toolPermissionContent, argument('content'))],
    'edit_file' || 'edit' => [
      detail(
        l10n.toolPermissionOldText,
        argument('oldString') ?? argument('old_string'),
      ),
      detail(
        l10n.toolPermissionNewText,
        argument('newString') ?? argument('new_string'),
      ),
    ],
    'execute_command' || 'bash' => [
      detail(l10n.toolPermissionCommand, argument('command')),
      detail(
        l10n.toolPermissionTerminalId,
        argument('terminal_id') ?? argument('terminalId'),
      ),
      detail(l10n.toolPermissionInput, argument('input')),
    ],
    'send_terminal_input' => [
      detail(
        l10n.toolPermissionTerminalId,
        argument('terminal_id') ?? argument('terminalId'),
      ),
      detail(l10n.toolPermissionInput, argument('input')),
    ],
    'web_search' => [
      detail(l10n.toolSearchQuery, argument('query')),
      detail(l10n.toolPermissionLimit, scalar(request.arguments['limit'] ?? 5)),
    ],
    'read_url_content' || 'read_url' => [
      detail(l10n.toolUrl, argument('url')),
      detail(
        l10n.toolPermissionLimit,
        scalar(
          request.arguments['max_chars'] ??
              request.arguments['maxChars'] ??
              6000,
        ),
      ),
    ],
    _ => const <({String label, String value})?>[],
  };
  return operationDetails.whereType<({String label, String value})>().toList();
}
