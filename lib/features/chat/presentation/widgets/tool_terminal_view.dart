import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolTerminalData {
  const ToolTerminalData({
    this.command,
    this.terminalId,
    this.input,
    this.output,
    this.isRunning = false,
    this.waitingForInput = false,
    this.exitCode,
    this.isTerminated = false,
    this.isTruncated = false,
    this.errorMessage,
  });

  final String? command;
  final String? terminalId;
  final String? input;
  final String? output;
  final bool isRunning;
  final bool waitingForInput;
  final int? exitCode;
  final bool isTerminated;
  final bool isTruncated;
  final String? errorMessage;

  static ToolTerminalData fromActivity(ChatToolActivity activity) {
    final arguments = toolActivityObjectMap(activity.arguments);
    final outputMap = toolActivityObjectMap(activity.output);

    final rawCommand = arguments?['command'] ?? outputMap?['command'];
    final command = rawCommand is String ? rawCommand : null;

    final rawTermId =
        arguments?['terminal_id'] ??
        arguments?['terminalId'] ??
        outputMap?['terminal_id'] ??
        outputMap?['terminalId'];
    final terminalId = rawTermId is String ? rawTermId : null;

    final rawInput = arguments?['input'];
    final input = rawInput is String ? rawInput : null;

    final rawOutput = outputMap?['output'];
    final output = rawOutput is String ? rawOutput : null;

    final isRunning =
        outputMap?['is_running'] == true ||
        activity.status == ChatToolActivityStatus.running;

    final waitingForInput =
        outputMap?['waiting_for_input'] == true ||
        (isRunning && (outputMap?['waiting_for_input'] ?? true) == true);

    final rawExitCode = outputMap?['exit_code'];
    final exitCode = rawExitCode is int ? rawExitCode : null;

    final isTerminated =
        outputMap?['status'] == 'terminated' || outputMap?['is_killed'] == true;

    final isTruncated = outputMap?['truncated'] == true;

    final errorObj = toolActivityObjectMap(outputMap?['error']);
    final errorMessage = errorObj?['message'] is String
        ? errorObj!['message'] as String
        : null;

    return ToolTerminalData(
      command: command,
      terminalId: terminalId,
      input: input,
      output: output,
      isRunning: isRunning,
      waitingForInput: waitingForInput,
      exitCode: exitCode,
      isTerminated: isTerminated,
      isTruncated: isTruncated,
      errorMessage: errorMessage,
    );
  }
}

String cleanAnsiCodes(String text) =>
    text.replaceAll(RegExp(r'\x1B\[[0-9;]*[a-zA-Z]'), '');

class ToolTerminalResult extends StatelessWidget {
  const ToolTerminalResult({
    super.key,
    required this.data,
    required this.palette,
  });

  final ToolTerminalData data;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);
    final cleanedOutput = cleanAnsiCodes(data.output ?? '').trim();

    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                Icons.terminal_rounded,
                size: 15,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  data.terminalId != null
                      ? 'terminal: ${data.terminalId}'
                      : l10n.toolExecuteCommand,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 11,
                    height: 16 / 11,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              _TerminalStatusBadge(data: data, palette: palette),
              if (cleanedOutput.isNotEmpty || data.command != null) ...[
                const SizedBox(width: 6),
                IconButton(
                  tooltip: l10n.toolTerminalCopied,
                  iconSize: 14,
                  visualDensity: VisualDensity.compact,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 24,
                    minHeight: 24,
                  ),
                  icon: Icon(Icons.copy_rounded, color: palette.secondaryIcon),
                  onPressed: () {
                    final textToCopy = cleanedOutput.isNotEmpty
                        ? cleanedOutput
                        : data.command ?? '';
                    Clipboard.setData(ClipboardData(text: textToCopy));
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(
                        content: Text(l10n.toolTerminalCopied),
                        duration: const Duration(seconds: 2),
                      ),
                    );
                  },
                ),
              ],
            ],
          ),
          if (data.command case final command?) ...[
            const SizedBox(height: 7),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
              decoration: BoxDecoration(
                color: palette.composer,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: palette.border),
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '\$',
                    style: TextStyle(
                      color: palette.accent,
                      fontFamily: 'monospace',
                      fontSize: 12,
                      fontWeight: FontWeight.w700,
                      height: 18 / 12,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: SelectableText(
                      command,
                      style: TextStyle(
                        color: palette.text,
                        fontFamily: 'monospace',
                        fontSize: 12,
                        height: 18 / 12,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          if (data.input case final input?) ...[
            const SizedBox(height: 5),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: BoxDecoration(
                color: palette.composer.withValues(alpha: 0.6),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: palette.border.withValues(alpha: 0.6),
                ),
              ),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '>',
                    style: TextStyle(
                      color: palette.secondaryIcon,
                      fontFamily: 'monospace',
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      height: 18 / 12,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: SelectableText(
                      input.trimRight(),
                      style: TextStyle(
                        color: palette.text,
                        fontFamily: 'monospace',
                        fontSize: 12,
                        height: 18 / 12,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 7),
          Container(
            decoration: BoxDecoration(
              color: palette.surface,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: palette.border),
            ),
            constraints: const BoxConstraints(maxHeight: 240, minHeight: 44),
            padding: const EdgeInsets.all(10),
            child: SingleChildScrollView(
              child: Align(
                alignment: Alignment.topLeft,
                child: data.errorMessage != null
                    ? Text(
                        data.errorMessage!,
                        style: TextStyle(
                          color: theme.colorScheme.error,
                          fontFamily: 'monospace',
                          fontSize: 12,
                          height: 18 / 12,
                        ),
                      )
                    : cleanedOutput.isNotEmpty
                    ? SelectableText(
                        cleanedOutput,
                        style: TextStyle(
                          color: palette.text,
                          fontFamily: 'monospace',
                          fontSize: 12,
                          height: 18 / 12,
                        ),
                      )
                    : data.isRunning
                    ? Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          SizedBox.square(
                            dimension: 12,
                            child: CircularProgressIndicator(
                              strokeWidth: 1.5,
                              color: palette.accent,
                            ),
                          ),
                          const SizedBox(width: 8),
                          Text(
                            l10n.toolTerminalWaitingOutput,
                            style: TextStyle(
                              color: palette.secondaryText,
                              fontSize: 11,
                              fontStyle: FontStyle.italic,
                            ),
                          ),
                        ],
                      )
                    : Text(
                        l10n.toolTerminalNoOutput,
                        style: TextStyle(
                          color: palette.secondaryText,
                          fontSize: 11,
                          fontStyle: FontStyle.italic,
                        ),
                      ),
              ),
            ),
          ),
          if (data.isTruncated) ...[
            const SizedBox(height: 6),
            Row(
              children: [
                Icon(
                  Icons.info_outline_rounded,
                  size: 13,
                  color: palette.secondaryIcon,
                ),
                const SizedBox(width: 5),
                Expanded(
                  child: Text(
                    l10n.toolListingIncomplete,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 11,
                      height: 16 / 11,
                    ),
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

class _TerminalStatusBadge extends StatelessWidget {
  const _TerminalStatusBadge({required this.data, required this.palette});

  final ToolTerminalData data;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final theme = Theme.of(context);

    if (data.isTerminated) {
      return Container(
        padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
        decoration: BoxDecoration(
          color: palette.composer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          l10n.toolTerminalTerminated,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 10,
            fontWeight: FontWeight.w500,
          ),
        ),
      );
    }

    if (data.isRunning) {
      return Container(
        padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
        decoration: BoxDecoration(
          color: palette.composer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox.square(
              dimension: 8,
              child: CircularProgressIndicator(
                strokeWidth: 1.2,
                color: palette.accent,
              ),
            ),
            const SizedBox(width: 5),
            Text(
              data.waitingForInput
                  ? l10n.toolTerminalWaitingForInput
                  : l10n.toolTerminalRunning,
              style: TextStyle(
                color: palette.accent,
                fontSize: 10,
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
      );
    }

    if (data.exitCode case final code?) {
      final isSuccess = code == 0;
      final badgeColor = isSuccess
          ? palette.secondaryIcon
          : theme.colorScheme.error;
      return Container(
        padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
        decoration: BoxDecoration(
          color: palette.composer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          l10n.toolTerminalExitCode(code),
          style: TextStyle(
            color: badgeColor,
            fontSize: 10,
            fontWeight: FontWeight.w600,
          ),
        ),
      );
    }

    return const SizedBox.shrink();
  }
}
