import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:openchat/l10n/openchat_localizations.dart';

Future<void> copyTextToClipboard(BuildContext context, String value) async {
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

Future<void> openUrlOrCopy(BuildContext context, String rawUrl) async {
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
    await copyTextToClipboard(context, rawUrl);
    return;
  }

  try {
    final result = await Process.run(executable, [uri.toString()]);
    if (result.exitCode != 0 && context.mounted) {
      await copyTextToClipboard(context, rawUrl);
    }
  } on ProcessException {
    if (!context.mounted) return;
    await copyTextToClipboard(context, rawUrl);
  }
}
