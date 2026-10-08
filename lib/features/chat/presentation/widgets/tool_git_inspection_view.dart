import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/tool_file_listing.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolGitInspectionResult extends StatelessWidget {
  const ToolGitInspectionResult({
    super.key,
    required this.activity,
    required this.palette,
  });

  final ChatToolActivity activity;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
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
      return ToolActivityNotice(
        message: l10n.toolOperationFailed,
        palette: palette,
        isError: true,
      );
    }

    return switch (activity.name) {
      'git_status' => _status(context, output),
      'git_diff' => _diff(context, output),
      'git_history' => _history(context, output),
      _ => ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      ),
    };
  }

  Widget _status(BuildContext context, Map<String, Object?>? output) {
    final l10n = context.openchatL10n;
    final branch = output?['branch'];
    final upstream = output?['upstream'];
    final files = output?['files'];
    if (output == null ||
        (branch != null && branch is! String) ||
        (upstream != null && upstream is! String) ||
        files is! List) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    final parsedFiles =
        <({String path, String status, bool staged, bool unstaged})>[];
    for (final raw in files) {
      final file = toolActivityObjectMap(raw);
      final path = file?['path'];
      final status = file?['status'];
      final staged = file?['staged'];
      final unstaged = file?['unstaged'];
      if (path is! String ||
          status is! String ||
          staged is! bool ||
          unstaged is! bool) {
        return ToolActivityNotice(
          message: l10n.toolOperationUnavailable,
          palette: palette,
        );
      }
      parsedFiles.add((
        path: path,
        status: status,
        staged: staged,
        unstaged: unstaged,
      ));
    }

    final ahead = output['ahead'];
    final behind = output['behind'];
    if ((ahead != null && ahead is! int) ||
        (behind != null && behind is! int)) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            _GitMeta(
              icon: LucideIcons.gitBranch,
              label: l10n.toolGitBranch,
              value: branch is String && branch.isNotEmpty ? branch : '—',
              palette: palette,
            ),
            if (upstream is String && upstream.isNotEmpty)
              _GitMeta(
                icon: LucideIcons.gitPullRequest,
                label: l10n.toolGitUpstream,
                value: upstream,
                palette: palette,
              ),
            if (ahead is int && ahead > 0)
              _GitCount(
                icon: LucideIcons.arrowUp,
                label: l10n.toolGitAhead,
                count: ahead,
                palette: palette,
              ),
            if (behind is int && behind > 0)
              _GitCount(
                icon: LucideIcons.arrowDown,
                label: l10n.toolGitBehind,
                count: behind,
                palette: palette,
              ),
          ],
        ),
        const SizedBox(height: 8),
        if (parsedFiles.isEmpty)
          ToolActivityNotice(
            message: output['truncated'] == true
                ? l10n.toolOperationTruncated
                : l10n.toolGitNoChanges,
            palette: palette,
          )
        else ...[
          for (var index = 0; index < parsedFiles.length; index++) ...[
            if (index > 0) Divider(height: 1, color: palette.border),
            _GitFileRow(file: parsedFiles[index], palette: palette),
          ],
        ],
        if (output['truncated'] == true) ...[
          const SizedBox(height: 6),
          Text(
            l10n.toolOperationTruncated,
            style: TextStyle(color: palette.secondaryText, fontSize: 11),
          ),
        ],
      ],
    );
  }

  Widget _diff(BuildContext context, Map<String, Object?>? output) {
    final l10n = context.openchatL10n;
    final staged = output?['staged'];
    final unstaged = output?['unstaged'];
    final stagedTruncated = output?['stagedTruncated'];
    final unstagedTruncated = output?['unstagedTruncated'];
    if (output == null ||
        staged is! String ||
        unstaged is! String ||
        stagedTruncated is! bool ||
        unstagedTruncated is! bool) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }
    if (staged.isEmpty && unstaged.isEmpty) {
      return ToolActivityNotice(message: l10n.toolGitNoDiff, palette: palette);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (staged.isNotEmpty)
          _GitDiffSection(
            title: l10n.toolGitStaged,
            value: staged,
            truncated: stagedTruncated,
            palette: palette,
          ),
        if (staged.isNotEmpty && unstaged.isNotEmpty)
          Divider(height: 16, color: palette.border),
        if (unstaged.isNotEmpty)
          _GitDiffSection(
            title: l10n.toolGitUnstaged,
            value: unstaged,
            truncated: unstagedTruncated,
            palette: palette,
          ),
      ],
    );
  }

  Widget _history(BuildContext context, Map<String, Object?>? output) {
    final l10n = context.openchatL10n;
    final entries = output?['entries'];
    if (output == null || entries is! List<Object?>) {
      return ToolActivityNotice(
        message: l10n.toolOperationUnavailable,
        palette: palette,
      );
    }
    final parsedEntries = <({String id, int timestamp, String subject})>[];
    for (final raw in entries) {
      final entry = toolActivityObjectMap(raw);
      final id = entry?['id'];
      final timestamp = entry?['timestampUnixSeconds'];
      final subject = entry?['subject'];
      if (id is! String || timestamp is! int || subject is! String) {
        return ToolActivityNotice(
          message: l10n.toolOperationUnavailable,
          palette: palette,
        );
      }
      parsedEntries.add((id: id, timestamp: timestamp, subject: subject));
    }
    if (parsedEntries.isEmpty) {
      return ToolActivityNotice(
        message: output['truncated'] == true
            ? l10n.toolOperationTruncated
            : l10n.toolGitNoHistory,
        palette: palette,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (var index = 0; index < parsedEntries.length; index++) ...[
          if (index > 0) Divider(height: 1, color: palette.border),
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 7),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(
                  width: 58,
                  child: Text(
                    parsedEntries[index].id.substring(
                      0,
                      parsedEntries[index].id.length.clamp(0, 8),
                    ),
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontFamily: 'monospace',
                      fontSize: 10,
                    ),
                  ),
                ),
                Expanded(
                  child: Text(
                    parsedEntries[index].subject,
                    style: TextStyle(color: palette.text, fontSize: 12),
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  DateTime.fromMillisecondsSinceEpoch(
                    parsedEntries[index].timestamp * 1000,
                    isUtc: true,
                  ).toLocal().toString().substring(0, 16),
                  style: TextStyle(color: palette.secondaryText, fontSize: 10),
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

class _GitMeta extends StatelessWidget {
  const _GitMeta({
    required this.icon,
    required this.label,
    required this.value,
    required this.palette,
  });

  final IconData icon;
  final String label;
  final String value;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Icon(icon, size: 13, color: palette.secondaryIcon),
      const SizedBox(width: 5),
      Text(
        '$label: ',
        style: TextStyle(color: palette.secondaryText, fontSize: 11),
      ),
      ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 220),
        child: Text(
          value,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: palette.text,
            fontFamily: 'monospace',
            fontSize: 11,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
    ],
  );
}

class _GitCount extends StatelessWidget {
  const _GitCount({
    required this.icon,
    required this.label,
    required this.count,
    required this.palette,
  });

  final IconData icon;
  final String label;
  final int count;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Icon(icon, size: 12, color: palette.accentIcon),
      const SizedBox(width: 3),
      Text(
        '$label $count',
        style: TextStyle(color: palette.text, fontSize: 11),
      ),
    ],
  );
}

class _GitFileRow extends StatelessWidget {
  const _GitFileRow({required this.file, required this.palette});

  final ({String path, String status, bool staged, bool unstaged}) file;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 1),
            child: SizedBox(
              width: 28,
              child: Text(
                file.status,
                style: TextStyle(
                  color: palette.accentIcon,
                  fontFamily: 'monospace',
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SelectableText(
                  file.path,
                  style: TextStyle(
                    color: palette.text,
                    fontFamily: 'monospace',
                    fontSize: 11,
                  ),
                ),
                if (file.staged || file.unstaged) ...[
                  const SizedBox(height: 4),
                  Wrap(
                    spacing: 4,
                    runSpacing: 4,
                    children: [
                      if (file.staged)
                        _GitTag(label: l10n.toolGitStaged, palette: palette),
                      if (file.unstaged)
                        _GitTag(label: l10n.toolGitUnstaged, palette: palette),
                    ],
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _GitTag extends StatelessWidget {
  const _GitTag({required this.label, required this.palette});

  final String label;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
    decoration: BoxDecoration(
      color: palette.surface,
      borderRadius: BorderRadius.circular(6),
    ),
    child: Text(
      label,
      style: TextStyle(color: palette.secondaryText, fontSize: 9),
    ),
  );
}

class _GitDiffSection extends StatelessWidget {
  const _GitDiffSection({
    required this.title,
    required this.value,
    required this.truncated,
    required this.palette,
  });

  final String title;
  final String value;
  final bool truncated;
  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Padding(
        padding: const EdgeInsets.only(bottom: 5),
        child: Text(
          title,
          style: TextStyle(
            color: palette.secondaryText,
            fontSize: 11,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
      Container(
        width: double.infinity,
        constraints: const BoxConstraints(maxHeight: 220),
        padding: const EdgeInsets.all(8),
        decoration: BoxDecoration(
          color: palette.surface,
          border: Border.all(color: palette.border),
          borderRadius: BorderRadius.circular(8),
        ),
        child: SingleChildScrollView(
          child: SelectableText(
            value,
            style: TextStyle(
              color: palette.text,
              fontFamily: 'monospace',
              fontSize: 10,
              height: 1.45,
            ),
          ),
        ),
      ),
      if (truncated) ...[
        const SizedBox(height: 4),
        Text(
          context.openchatL10n.toolOperationTruncated,
          style: TextStyle(color: palette.secondaryText, fontSize: 10),
        ),
      ],
    ],
  );
}
