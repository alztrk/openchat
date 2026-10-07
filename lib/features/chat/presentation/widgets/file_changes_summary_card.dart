import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class FileChangesSummaryCard extends StatelessWidget {
  const FileChangesSummaryCard({
    required this.message,
    required this.onViewChanges,
    this.latestChanges = const <ChatFileChange>[],
    super.key,
  });

  final ChatMessage message;
  final VoidCallback? onViewChanges;
  final List<ChatFileChange> latestChanges;

  @override
  Widget build(BuildContext context) {
    final activities = message.toolActivities;
    final changesById = <String, ChatFileChange>{};
    for (final activity in activities) {
      for (final change in activity.fileChanges) {
        changesById[change.id] = change;
      }
    }
    for (final change in latestChanges) {
      if (changesById.containsKey(change.id)) {
        changesById[change.id] = change;
      }
    }
    final changes = changesById.values.toList(growable: false);
    final activeChanges = changes
        .where((change) => change.status == ChatFileChangeState.active)
        .toList(growable: false);
    String? trackingError;
    for (final activity in activities) {
      if (activity.fileChangesError case final error?) trackingError = error;
    }
    if (changes.isEmpty && trackingError == null) {
      return const SizedBox.shrink();
    }

    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final added = activeChanges.fold<int>(
      0,
      (total, change) => total + (change.addedLines ?? 0),
    );
    final removed = activeChanges.fold<int>(
      0,
      (total, change) => total + (change.removedLines ?? 0),
    );
    final hasUnknownStats = activeChanges.any(
      (change) => change.addedLines == null || change.removedLines == null,
    );

    return Container(
      margin: const EdgeInsets.only(top: 10),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border.all(color: palette.border),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 10, 10, 8),
            child: Row(
              children: [
                Container(
                  width: 30,
                  height: 30,
                  decoration: BoxDecoration(
                    color: palette.composer,
                    borderRadius: BorderRadius.circular(9),
                  ),
                  alignment: Alignment.center,
                  child: Icon(
                    LucideIcons.fileDiff,
                    size: 17,
                    color: palette.accentIcon,
                  ),
                ),
                const SizedBox(width: 9),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        trackingError != null && activeChanges.isEmpty
                            ? l10n.fileChangesUnavailableTitle
                            : l10n.fileChangesSummary(
                                activeChanges.isEmpty
                                    ? changes.length
                                    : activeChanges.length,
                              ),
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          height: 18 / 13,
                        ),
                      ),
                      if (activeChanges.isNotEmpty)
                        Text(
                          hasUnknownStats
                              ? l10n.fileChangesSomeCountsUnavailable
                              : l10n.fileChangesLineCounts(added, removed),
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: 11,
                            height: 16 / 11,
                          ),
                        ),
                    ],
                  ),
                ),
                if (onViewChanges != null)
                  TextButton(
                    onPressed: onViewChanges,
                    child: Text(l10n.fileChangesView),
                  ),
              ],
            ),
          ),
          if (activeChanges.isNotEmpty) ...[
            Divider(height: 1, color: palette.border),
            for (final change in activeChanges.take(3))
              Padding(
                padding: const EdgeInsets.fromLTRB(13, 8, 13, 8),
                child: Row(
                  children: [
                    Icon(
                      _fileChangeIcon(change.kind),
                      size: 15,
                      color: palette.secondaryIcon,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        change.path,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 12,
                          height: 18 / 12,
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    if (change.addedLines case final addedLines?)
                      Text(
                        '+$addedLines',
                        style: TextStyle(
                          color: _additionColor(context),
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    if (change.removedLines case final removedLines?) ...[
                      const SizedBox(width: 7),
                      Text(
                        '-$removedLines',
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            if (activeChanges.length > 3)
              Padding(
                padding: const EdgeInsets.fromLTRB(13, 0, 13, 10),
                child: Text(
                  l10n.fileChangesMoreFiles(activeChanges.length - 3),
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 11,
                    height: 16 / 11,
                  ),
                ),
              ),
          ],
          if (activeChanges.isEmpty && changes.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(13, 4, 13, 10),
              child: Text(
                changes.any(
                      (change) => change.status == ChatFileChangeState.conflict,
                    )
                    ? l10n.fileChangesConflict
                    : l10n.fileChangesReverted,
                style: TextStyle(
                  color: palette.secondaryText,
                  fontSize: 11,
                  height: 16 / 11,
                ),
              ),
            ),
          if (trackingError != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(13, 4, 13, 11),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    LucideIcons.info,
                    size: 15,
                    color: Theme.of(context).colorScheme.error,
                  ),
                  const SizedBox(width: 7),
                  Expanded(
                    child: Text(
                      l10n.fileChangesTrackingFailed,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                        fontSize: 11,
                        height: 16 / 11,
                      ),
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

IconData _fileChangeIcon(ChatFileChangeKind kind) => switch (kind) {
  ChatFileChangeKind.added => LucideIcons.filePlus,
  ChatFileChangeKind.modified => LucideIcons.notebookPen,
  ChatFileChangeKind.deleted => LucideIcons.trash2,
};

Color _additionColor(BuildContext context) =>
    Theme.of(context).colorScheme.tertiary;
