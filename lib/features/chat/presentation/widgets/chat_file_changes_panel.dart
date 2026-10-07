import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_file_changes_repository.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ChatFileChangesPanel extends StatefulWidget {
  const ChatFileChangesPanel({
    required this.repository,
    required this.conversationId,
    this.refreshRevision = 0,
    required this.onClose,
    required this.onChangesUpdated,
    super.key,
  });

  final ChatFileChangesRepository repository;
  final String conversationId;
  final int refreshRevision;
  final VoidCallback onClose;
  final ValueChanged<List<ChatFileChange>> onChangesUpdated;

  @override
  State<ChatFileChangesPanel> createState() => _ChatFileChangesPanelState();
}

class _ChatFileChangesPanelState extends State<ChatFileChangesPanel> {
  List<ChatFileChange> _changes = const <ChatFileChange>[];
  ChatFileChangeDiff? _selectedDiff;
  String? _selectedChangeId;
  String? _error;
  bool _isLoading = true;
  bool _isLoadingDiff = false;
  String? _revertingChangeId;
  int _changesRequest = 0;
  int _diffRequest = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadChanges());
  }

  @override
  void didUpdateWidget(covariant ChatFileChangesPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.conversationId != widget.conversationId ||
        oldWidget.refreshRevision != widget.refreshRevision) {
      unawaited(_loadChanges());
    }
  }

  Future<void> _loadChanges({String? selectChangeId}) async {
    final request = ++_changesRequest;
    final conversationId = widget.conversationId;
    final previousSelection = _selectedChangeId;
    setState(() {
      _isLoading = true;
      _error = null;
      _selectedDiff = null;
      _isLoadingDiff = false;
      _selectedChangeId = selectChangeId;
    });
    try {
      final changes = await widget.repository.list(conversationId);
      if (!mounted ||
          request != _changesRequest ||
          conversationId != widget.conversationId) {
        return;
      }
      final selected =
          selectChangeId ??
          (changes.any((change) => change.id == previousSelection)
              ? previousSelection
              : _firstActiveChange(changes)?.id);
      setState(() {
        _changes = changes;
        _selectedChangeId = selected;
        _isLoading = false;
      });
      widget.onChangesUpdated(changes);
      if (selected != null) unawaited(_loadDiff(selected));
    } on OpenChatServiceException {
      if (!mounted ||
          request != _changesRequest ||
          conversationId != widget.conversationId) {
        return;
      }
      setState(() {
        _isLoading = false;
        _error = 'load';
      });
    } on FormatException {
      if (!mounted ||
          request != _changesRequest ||
          conversationId != widget.conversationId) {
        return;
      }
      setState(() {
        _isLoading = false;
        _error = 'load';
      });
    }
  }

  Future<void> _loadDiff(String changeId) async {
    final request = ++_diffRequest;
    final conversationId = widget.conversationId;
    setState(() {
      _selectedChangeId = changeId;
      _selectedDiff = null;
      _isLoadingDiff = true;
      _error = null;
    });
    try {
      final diff = await widget.repository.diff(
        conversationId: conversationId,
        changeId: changeId,
      );
      if (!mounted ||
          request != _diffRequest ||
          conversationId != widget.conversationId ||
          _selectedChangeId != changeId) {
        return;
      }
      setState(() {
        _selectedDiff = diff;
        _isLoadingDiff = false;
      });
    } on OpenChatServiceException {
      if (!mounted ||
          request != _diffRequest ||
          conversationId != widget.conversationId ||
          _selectedChangeId != changeId) {
        return;
      }
      setState(() {
        _isLoadingDiff = false;
        _error = 'diff';
      });
    } on FormatException {
      if (!mounted ||
          request != _diffRequest ||
          conversationId != widget.conversationId ||
          _selectedChangeId != changeId) {
        return;
      }
      setState(() {
        _isLoadingDiff = false;
        _error = 'diff';
      });
    }
  }

  Future<void> _revert(ChatFileChange change) async {
    if (!change.canRevert || _revertingChangeId != null) return;
    final conversationId = widget.conversationId;
    setState(() {
      _revertingChangeId = change.id;
      _error = null;
    });
    try {
      final updated = await widget.repository.revert(
        conversationId: conversationId,
        changeId: change.id,
      );
      if (!mounted || conversationId != widget.conversationId) return;
      setState(() {
        _changes = updated;
        _revertingChangeId = null;
      });
      widget.onChangesUpdated(updated);
      unawaited(_loadDiff(change.id));
    } on OpenChatServiceException catch (error) {
      if (!mounted || conversationId != widget.conversationId) return;
      setState(() {
        _revertingChangeId = null;
        _error = error.code == 'file_change_conflict' ? 'conflict' : 'revert';
      });
      if (error.code == 'file_change_conflict') {
        unawaited(_loadChanges(selectChangeId: change.id));
      }
    } on FormatException {
      if (!mounted) return;
      setState(() {
        _revertingChangeId = null;
        _error = 'revert';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final active = _changes
        .where((change) => change.status == ChatFileChangeState.active)
        .toList(growable: false);
    final added = active.fold<int>(
      0,
      (total, change) => total + (change.addedLines ?? 0),
    );
    final removed = active.fold<int>(
      0,
      (total, change) => total + (change.removedLines ?? 0),
    );

    return Material(
      color: palette.surface,
      child: Container(
        decoration: BoxDecoration(
          border: Border(left: BorderSide(color: palette.border)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 13, 10, 11),
              child: Row(
                children: [
                  Icon(
                    LucideIcons.fileDiff,
                    size: 17,
                    color: palette.accentIcon,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      l10n.fileChangesTitle,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        height: 20 / 14,
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: l10n.close,
                    onPressed: widget.onClose,
                    icon: const Icon(LucideIcons.x, size: 17),
                  ),
                ],
              ),
            ),
            Divider(height: 1, color: palette.border),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
              child: Row(
                children: [
                  Text(
                    l10n.fileChangesSummary(active.length),
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const Spacer(),
                  _LineCount(label: '+$added', color: _additionColor(context)),
                  const SizedBox(width: 8),
                  _LineCount(
                    label: '-$removed',
                    color: Theme.of(context).colorScheme.error,
                  ),
                ],
              ),
            ),
            if (_error case final error?)
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
                child: _InlineMessage(
                  message: switch (error) {
                    'load' => l10n.fileChangesLoadFailed,
                    'diff' => l10n.fileChangesDiffFailed,
                    'conflict' => l10n.fileChangesConflict,
                    _ => l10n.fileChangesRevertFailed,
                  },
                  isError: true,
                ),
              ),
            Expanded(
              child: _isLoading
                  ? Center(
                      child: SizedBox.square(
                        dimension: 20,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: palette.accent,
                        ),
                      ),
                    )
                  : _changes.isEmpty
                  ? Center(
                      child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Text(
                          l10n.fileChangesEmpty,
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            color: palette.secondaryText,
                            fontSize: 12,
                            height: 18 / 12,
                          ),
                        ),
                      ),
                    )
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Flexible(
                          flex: _selectedChangeId == null ? 1 : 4,
                          child: ListView.separated(
                            padding: const EdgeInsets.symmetric(vertical: 4),
                            itemCount: _changes.length,
                            separatorBuilder: (context, index) => Divider(
                              height: 1,
                              indent: 14,
                              endIndent: 14,
                              color: palette.border.withValues(alpha: 0.7),
                            ),
                            itemBuilder: (context, index) => _FileChangeRow(
                              change: _changes[index],
                              isSelected:
                                  _changes[index].id == _selectedChangeId,
                              isReverting:
                                  _changes[index].id == _revertingChangeId,
                              onSelect: () =>
                                  unawaited(_loadDiff(_changes[index].id)),
                              onRevert: () =>
                                  unawaited(_revert(_changes[index])),
                            ),
                          ),
                        ),
                        if (_selectedChangeId != null) ...[
                          Divider(height: 1, color: palette.border),
                          SizedBox(
                            height: 38,
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 13,
                                vertical: 9,
                              ),
                              child: Text(
                                _selectedDiff?.path ??
                                    _selectedChangePath ??
                                    l10n.fileChangesDiffTitle,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  color: palette.text,
                                  fontSize: 11,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ),
                          ),
                          Expanded(
                            flex: 5,
                            child: _buildDiff(context, l10n, palette),
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

  Widget _buildDiff(
    BuildContext context,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    if (_isLoadingDiff) {
      return Center(
        child: SizedBox.square(
          dimension: 18,
          child: CircularProgressIndicator(
            strokeWidth: 1.8,
            color: palette.accent,
          ),
        ),
      );
    }
    final diff = _selectedDiff;
    if (diff == null) {
      return const SizedBox.shrink();
    }
    if (diff.isBinary) {
      return _DiffNotice(message: l10n.fileChangesBinary, palette: palette);
    }
    if (!diff.available) {
      return _DiffNotice(
        message: l10n.fileChangesDiffUnavailable,
        palette: palette,
      );
    }

    final lines = diff.diff.split('\n');
    return ListView.builder(
      key: ValueKey<String>('file-change-diff-${diff.path}'),
      itemCount: lines.length + (diff.isTruncated ? 1 : 0),
      itemBuilder: (context, index) {
        if (index == lines.length) {
          return Padding(
            padding: const EdgeInsets.all(10),
            child: Text(
              l10n.fileChangesDiffTruncated,
              style: TextStyle(color: palette.secondaryText, fontSize: 11),
            ),
          );
        }
        final line = lines[index];
        final color = line.startsWith('+') && !line.startsWith('+++')
            ? _additionColor(context)
            : line.startsWith('-') && !line.startsWith('---')
            ? Theme.of(context).colorScheme.error
            : palette.secondaryText;
        return Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 1),
          child: SelectableText(
            line.isEmpty ? ' ' : line,
            style: TextStyle(
              color: color,
              fontFamily: 'monospace',
              fontSize: 11,
              height: 16 / 11,
            ),
          ),
        );
      },
    );
  }

  String? get _selectedChangePath {
    for (final change in _changes) {
      if (change.id == _selectedChangeId) return change.path;
    }
    return null;
  }
}

class _FileChangeRow extends StatelessWidget {
  const _FileChangeRow({
    required this.change,
    required this.isSelected,
    required this.isReverting,
    required this.onSelect,
    required this.onRevert,
  });

  final ChatFileChange change;
  final bool isSelected;
  final bool isReverting;
  final VoidCallback onSelect;
  final VoidCallback onRevert;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    return Material(
      color: isSelected ? palette.composer : Colors.transparent,
      child: InkWell(
        onTap: onSelect,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(13, 7, 10, 7),
          child: Row(
            children: [
              Icon(
                _fileChangeIcon(change.kind),
                size: 16,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      change.path,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 11,
                        fontWeight: FontWeight.w500,
                        height: 16 / 11,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      switch (change.status) {
                        ChatFileChangeState.active => l10n.fileChangesActive,
                        ChatFileChangeState.reverted =>
                          l10n.fileChangesReverted,
                        ChatFileChangeState.conflict =>
                          l10n.fileChangesConflict,
                      },
                      style: TextStyle(
                        color: change.status == ChatFileChangeState.conflict
                            ? Theme.of(context).colorScheme.error
                            : palette.secondaryText,
                        fontSize: 10,
                        height: 14 / 10,
                      ),
                    ),
                  ],
                ),
              ),
              if (change.addedLines != null || change.removedLines != null) ...[
                if (change.addedLines case final added?)
                  _LineCount(label: '+$added', color: _additionColor(context)),
                if (change.removedLines case final removed?) ...[
                  const SizedBox(width: 6),
                  _LineCount(
                    label: '-$removed',
                    color: Theme.of(context).colorScheme.error,
                  ),
                ],
                const SizedBox(width: 5),
              ] else
                const SizedBox(width: 5),
              if (change.canRevert)
                TextButton(
                  onPressed: isReverting ? null : onRevert,
                  style: TextButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    minimumSize: const Size(58, 30),
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  child: isReverting
                      ? const SizedBox.square(
                          dimension: 14,
                          child: CircularProgressIndicator(strokeWidth: 1.5),
                        )
                      : Text(l10n.fileChangesRevert),
                )
              else if (change.status == ChatFileChangeState.reverted)
                Icon(LucideIcons.check, size: 16, color: palette.secondaryIcon)
              else if (change.status == ChatFileChangeState.conflict)
                Tooltip(
                  message: l10n.fileChangesConflict,
                  child: Icon(
                    LucideIcons.triangleAlert,
                    size: 16,
                    color: Theme.of(context).colorScheme.error,
                  ),
                )
              else if (!change.diffAvailable)
                Tooltip(
                  message: l10n.fileChangesDiffUnavailable,
                  child: Icon(
                    LucideIcons.info,
                    size: 16,
                    color: palette.secondaryIcon,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _LineCount extends StatelessWidget {
  const _LineCount({required this.label, required this.color});

  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) => Text(
    label,
    style: TextStyle(
      color: color,
      fontSize: 11,
      fontWeight: FontWeight.w600,
      fontFamily: 'monospace',
    ),
  );
}

class _DiffNotice extends StatelessWidget {
  const _DiffNotice({required this.message, required this.palette});

  final String message;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(18),
      child: Text(
        message,
        textAlign: TextAlign.center,
        style: TextStyle(
          color: palette.secondaryText,
          fontSize: 12,
          height: 18 / 12,
        ),
      ),
    ),
  );
}

class _InlineMessage extends StatelessWidget {
  const _InlineMessage({required this.message, required this.isError});

  final String message;
  final bool isError;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
    decoration: BoxDecoration(
      color: isError
          ? Theme.of(context).colorScheme.error.withValues(alpha: 0.08)
          : null,
      borderRadius: BorderRadius.circular(8),
    ),
    child: Text(
      message,
      style: TextStyle(
        color: isError ? Theme.of(context).colorScheme.error : null,
        fontSize: 11,
        height: 16 / 11,
      ),
    ),
  );
}

ChatFileChange? _firstActiveChange(List<ChatFileChange> changes) {
  for (final change in changes) {
    if (change.status == ChatFileChangeState.active) return change;
  }
  return changes.isEmpty ? null : changes.first;
}

IconData _fileChangeIcon(ChatFileChangeKind kind) => switch (kind) {
  ChatFileChangeKind.added => LucideIcons.filePlus,
  ChatFileChangeKind.modified => LucideIcons.notebookPen,
  ChatFileChangeKind.deleted => LucideIcons.trash2,
};

Color _additionColor(BuildContext context) =>
    Theme.of(context).colorScheme.tertiary;
