import 'dart:convert';

import 'package:flutter/material.dart';

import '../../../../l10n/generated/app_localizations.dart';
import '../../../../l10n/openchat_localizations.dart';

enum ToolPermissionDecision { allow, deny, stop }

Future<ToolPermissionDecision?> showToolPermissionDialog({
  required BuildContext context,
  required String toolName,
  required String targetPath,
  required Object? arguments,
}) {
  final l10n = context.openchatL10n;
  final formattedArguments = const JsonEncoder.withIndent('  ')
      .convert(arguments);
  return showDialog<ToolPermissionDecision>(
    context: context,
    barrierDismissible: false,
    builder: (dialogContext) => AlertDialog(
      title: Text(l10n.toolPermissionRequestTitle),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(l10n.toolPermissionRequestDescription),
              const SizedBox(height: 20),
              Text(
                l10n.toolPermissionTool,
                style: Theme.of(dialogContext).textTheme.labelLarge,
              ),
              const SizedBox(height: 6),
              Text(_toolName(toolName, l10n)),
              const SizedBox(height: 16),
              Text(
                l10n.toolPermissionTarget,
                style: Theme.of(dialogContext).textTheme.labelLarge,
              ),
              const SizedBox(height: 6),
              SelectableText(targetPath),
              const SizedBox(height: 16),
              Text(
                l10n.toolPermissionArguments,
                style: Theme.of(dialogContext).textTheme.labelLarge,
              ),
              const SizedBox(height: 6),
              SelectableText(formattedArguments),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () =>
              Navigator.of(dialogContext).pop(ToolPermissionDecision.deny),
          child: Text(l10n.toolPermissionDeny),
        ),
        TextButton(
          onPressed: () =>
              Navigator.of(dialogContext).pop(ToolPermissionDecision.stop),
          child: Text(l10n.toolPermissionStopResponse),
        ),
        FilledButton(
          onPressed: () =>
              Navigator.of(dialogContext).pop(ToolPermissionDecision.allow),
          child: Text(l10n.toolPermissionAllowOnce),
        ),
      ],
    ),
  );
}

String _toolName(String name, AppLocalizations l10n) => switch (name) {
  'list_files' => l10n.toolListFiles,
  'search_files' => l10n.toolSearchFiles,
  'read_file' => l10n.toolReadFile,
  'get_file_info' => l10n.toolGetFileInfo,
  _ => name,
};
