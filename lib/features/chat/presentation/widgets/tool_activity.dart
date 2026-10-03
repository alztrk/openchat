import 'dart:convert';

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_surface_card.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_operation_results.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_terminal_view.dart';

class ToolActivityAccordion extends StatelessWidget {
  const ToolActivityAccordion({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final statusLabel = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval => l10n.toolAwaitingApproval,
      ChatToolActivityStatus.waitingForUser => l10n.toolWaitingForUser,
      ChatToolActivityStatus.running => l10n.toolRunning,
      ChatToolActivityStatus.completed => l10n.toolCompleted,
      ChatToolActivityStatus.failed => l10n.toolFailed,
      ChatToolActivityStatus.denied => l10n.toolDenied,
      ChatToolActivityStatus.cancelled => l10n.toolCancelled,
    };
    final statusIndicator = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval => Icon(
        Icons.lock_outline_rounded,
        size: 12,
        color: palette.accent,
      ),
      ChatToolActivityStatus.waitingForUser => Icon(
        Icons.question_answer_outlined,
        size: 12,
        color: palette.accent,
      ),
      ChatToolActivityStatus.running => SizedBox.square(
        dimension: 11,
        child: CircularProgressIndicator(
          strokeWidth: 1.5,
          color: palette.accent,
        ),
      ),
      ChatToolActivityStatus.completed => Icon(
        Icons.check_circle_outline_rounded,
        size: 12,
        color: palette.secondaryIcon,
      ),
      ChatToolActivityStatus.failed => Icon(
        Icons.error_outline_rounded,
        size: 12,
        color: Theme.of(context).colorScheme.error,
      ),
      ChatToolActivityStatus.denied => Icon(
        Icons.block_rounded,
        size: 12,
        color: Theme.of(context).colorScheme.error,
      ),
      ChatToolActivityStatus.cancelled => Icon(
        Icons.cancel_outlined,
        size: 12,
        color: palette.secondaryIcon,
      ),
    };
    final statusColor = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval ||
      ChatToolActivityStatus.waitingForUser ||
      ChatToolActivityStatus.running => palette.accent,
      ChatToolActivityStatus.failed ||
      ChatToolActivityStatus.denied => Theme.of(context).colorScheme.error,
      ChatToolActivityStatus.completed ||
      ChatToolActivityStatus.cancelled => palette.secondaryIcon,
    };
    final fileListing =
        activity.name == 'list_files' ||
            activity.name == 'glob' ||
            activity.name == 'list_directory'
        ? ToolFileListing.fromOutput(activity.output)
        : null;
    final fileListingOutput = toolActivityObjectMap(activity.output);
    final fileListingError = toolActivityObjectMap(
      fileListingOutput?['error'],
    )?['message'];
    final fileListingMessage = switch (activity.status) {
      ChatToolActivityStatus.awaitingApproval => l10n.toolAwaitingApproval,
      ChatToolActivityStatus.waitingForUser => l10n.toolWaitingForUser,
      ChatToolActivityStatus.running => l10n.toolOperationWorking,
      ChatToolActivityStatus.completed =>
        fileListingError is String && fileListingError.isNotEmpty
            ? fileListingError
            : l10n.toolListingUnavailable,
      ChatToolActivityStatus.failed =>
        fileListingError is String && fileListingError.isNotEmpty
            ? fileListingError
            : l10n.toolOperationFailed,
      ChatToolActivityStatus.denied => l10n.toolDenied,
      ChatToolActivityStatus.cancelled => l10n.toolCancelled,
    };
    final fileListingFailed =
        activity.status == ChatToolActivityStatus.failed ||
        fileListingOutput?['error'] != null;
    final operationResult = toolOperationResult(activity, palette);
    final isTerminal =
        activity.name == 'execute_command' ||
        activity.name == 'bash' ||
        activity.name == 'send_terminal_input';
    final terminalData = isTerminal
        ? ToolTerminalData.fromActivity(activity)
        : null;
    final cardRadius = BorderRadius.circular(OpenChatRadii.card);

    return ChatSurfaceCard(
      child: Material(
        color: Colors.transparent,
        child: Theme(
          data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
          child: ExpansionTile(
            key: ValueKey<String>('tool-${activity.callId}'),
            initiallyExpanded:
                activity.status == ChatToolActivityStatus.running ||
                activity.status == ChatToolActivityStatus.waitingForUser ||
                activity.status == ChatToolActivityStatus.awaitingApproval,
            tilePadding: const EdgeInsets.symmetric(horizontal: 10),
            childrenPadding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
            visualDensity: VisualDensity.compact,
            iconColor: palette.secondaryIcon,
            collapsedIconColor: palette.secondaryIcon,
            shape: RoundedRectangleBorder(borderRadius: cardRadius),
            collapsedShape: RoundedRectangleBorder(borderRadius: cardRadius),
            leading: Tooltip(
              message: _toolActivityName(activity.name, l10n),
              child: Semantics(
                label: _toolActivityName(activity.name, l10n),
                child: Container(
                  width: 28,
                  height: 28,
                  decoration: BoxDecoration(
                    color: palette.surface,
                    borderRadius: BorderRadius.circular(9),
                  ),
                  alignment: Alignment.center,
                  child: Icon(
                    toolOperationIcon(activity.name),
                    size: 16,
                    color: palette.accentIcon,
                  ),
                ),
              ),
            ),
            title: Row(
              children: [
                Expanded(
                  child: Text(
                    _toolActivityName(activity.name, l10n),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      height: 18 / 12,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 3,
                  ),
                  decoration: BoxDecoration(
                    color: palette.surface,
                    borderRadius: BorderRadius.circular(20),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      statusIndicator,
                      const SizedBox(width: 5),
                      Text(
                        statusLabel,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: statusColor,
                          fontSize: 10,
                          fontWeight: FontWeight.w500,
                          height: 14 / 10,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            children: [
              if (activity.name == 'list_files' ||
                  activity.name == 'glob' ||
                  activity.name == 'list_directory')
                if (fileListing == null)
                  ToolActivityNotice(
                    message: fileListingMessage,
                    palette: palette,
                    isError: fileListingFailed,
                    isLoading:
                        activity.status == ChatToolActivityStatus.running,
                  )
                else
                  ToolFileListingResult(
                    listing: fileListing,
                    locationLabel: _toolLocationLabel(activity, l10n),
                    palette: palette,
                  ),
              if (terminalData case final data?)
                ToolTerminalResult(data: data, palette: palette),
              if (operationResult case final Widget result) result,
              _ToolActivityTechnicalDetails(
                activity: activity,
                palette: palette,
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _toolActivityName(String name, AppLocalizations l10n) =>
      switch (name) {
        'list_files' || 'glob' || 'list_directory' => l10n.toolListFiles,
        'search_files' || 'grep' => l10n.toolSearchFiles,
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
}

class _ToolActivityTechnicalDetails extends StatelessWidget {
  const _ToolActivityTechnicalDetails({
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;

    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
        key: ValueKey<String>('tool-details-${activity.callId}'),
        tilePadding: const EdgeInsets.symmetric(horizontal: 2),
        childrenPadding: const EdgeInsets.fromLTRB(4, 0, 4, 8),
        visualDensity: VisualDensity.compact,
        iconColor: palette.secondaryIcon,
        collapsedIconColor: palette.secondaryIcon,
        title: Text(
          l10n.toolTechnicalDetails,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 11,
            fontWeight: FontWeight.w500,
            height: 16 / 11,
          ),
        ),
        leading: Icon(
          Icons.tune_rounded,
          size: 15,
          color: palette.secondaryIcon,
        ),
        children: [
          if (activity.targetPath case final targetPath?) ...[
            _ToolActivityValue(
              label: l10n.toolPermissionTarget,
              value: targetPath,
              palette: palette,
            ),
            const SizedBox(height: 8),
          ],
          _ToolActivityValue(
            label: l10n.toolInput,
            value: activity.arguments,
            palette: palette,
          ),
          if (activity.output != null) ...[
            const SizedBox(height: 8),
            _ToolActivityValue(
              label: l10n.toolOutput,
              value: activity.output,
              palette: palette,
            ),
          ],
        ],
      ),
    );
  }
}

String? _toolLocationLabel(ChatToolActivity activity, AppLocalizations l10n) {
  final arguments = toolActivityObjectMap(activity.arguments);
  final path = arguments?['path'];
  if (path is! String) return null;
  final normalized = path.trim().toLowerCase();
  if (normalized == 'desktop:/' || normalized == 'desktop:') {
    return l10n.toolDesktopLocation;
  }
  if (normalized == 'project:/' || normalized == 'project:') {
    return l10n.toolProjectLocation;
  }
  if (normalized == 'openchat:/' || normalized == 'openchat:') {
    return l10n.toolOpenChatLocation;
  }
  return null;
}

class _ToolActivityValue extends StatelessWidget {
  const _ToolActivityValue({
    required this.label,
    required this.value,
    required this.palette,
  });

  final String label;
  final Object? value;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final formattedValue = JsonEncoder.withIndent('  ').convert(value);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: 4),
          child: Text(
            label,
            style: TextStyle(
              color: palette.secondaryText,
              fontSize: 11,
              fontWeight: FontWeight.w500,
              height: 16 / 11,
            ),
          ),
        ),
        Container(
          width: double.infinity,
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: palette.composer,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: palette.border),
          ),
          child: SelectableText(
            formattedValue,
            style: TextStyle(
              color: palette.text,
              fontFamily: 'monospace',
              fontSize: 12,
              height: 18 / 12,
            ),
          ),
        ),
      ],
    );
  }
}
