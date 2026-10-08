import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProjectWorktreesDialog extends StatefulWidget {
  const ProjectWorktreesDialog({
    required this.serviceClient,
    required this.projectId,
    required this.projectRoot,
    required this.permissionMode,
    required this.projectPermissionRules,
    required this.onUseWorktree,
    super.key,
  });

  final OpenChatServiceClient serviceClient;
  final String projectId;
  final String projectRoot;
  final ToolPermissionMode permissionMode;
  final Map<String, ToolPermissionRule> projectPermissionRules;
  final Future<void> Function(String path, String branch) onUseWorktree;

  @override
  State<ProjectWorktreesDialog> createState() => _ProjectWorktreesDialogState();
}

class _ProjectWorktreesDialogState extends State<ProjectWorktreesDialog> {
  List<_ProjectWorktree> _worktrees = const <_ProjectWorktree>[];
  bool _isLoading = true;
  bool _isCreating = false;
  bool _isUsingWorktree = false;
  bool _truncated = false;
  String? _errorCode;
  String? _removingWorktreeId;
  String? _runningTaskWorktreeId;
  OpenChatServiceOperation? _runningTaskOperation;
  bool _isCancellingTask = false;

  @override
  void initState() {
    super.initState();
    _loadWorktrees();
  }

  Future<void> _loadWorktrees() async {
    setState(() {
      _isLoading = true;
      _errorCode = null;
    });
    try {
      final response = await widget.serviceClient.call(
        'project.worktrees.list',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
        },
      );
      final rawWorktrees = response['worktrees'];
      if (rawWorktrees is! List<Object?>) {
        throw const FormatException('The Git worktree list was invalid.');
      }
      final worktrees = rawWorktrees
          .map(_ProjectWorktree.fromObject)
          .toList(growable: false);
      final truncated = response['truncated'];
      if (truncated is! bool) {
        throw const FormatException('The Git worktree list was invalid.');
      }
      if (mounted) {
        setState(() {
          _worktrees = worktrees;
          _truncated = truncated;
          _isLoading = false;
        });
      }
    } on OpenChatServiceException catch (error) {
      if (mounted) {
        setState(() {
          _errorCode = error.code;
          _isLoading = false;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _errorCode = 'unavailable';
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _createWorktree() async {
    if (_isCreating || _isLoading) return;
    setState(() {
      _isCreating = true;
      _errorCode = null;
    });
    try {
      final response = await widget.serviceClient.call(
        'project.worktrees.create',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
        },
        timeout: const Duration(seconds: 50),
      );
      final worktree = _ProjectWorktree.fromObject(response);
      await _loadWorktrees();
      if (!mounted) return;
      await _useWorktree(worktree);
    } on OpenChatServiceException catch (error) {
      if (mounted) setState(() => _errorCode = error.code);
    } on Exception {
      if (mounted) setState(() => _errorCode = 'unavailable');
    } finally {
      if (mounted) setState(() => _isCreating = false);
    }
  }

  Future<void> _useWorktree(_ProjectWorktree worktree) async {
    if (_isUsingWorktree) return;
    setState(() => _isUsingWorktree = true);
    try {
      await widget.onUseWorktree(worktree.path, worktree.branch);
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (mounted) setState(() => _errorCode = 'unavailable');
    } finally {
      if (mounted) setState(() => _isUsingWorktree = false);
    }
  }

  Future<void> _reviewWorktree(_ProjectWorktree worktree) async {
    final l10n = context.openchatL10n;
    try {
      final response = await widget.serviceClient.call(
        'project.worktrees.review',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
          'worktreeId': worktree.id,
        },
      );
      final status = _readMap(response['status']);
      final diff = _readMap(response['diff']);
      if (!mounted) return;
      await showDialog<void>(
        context: context,
        builder: (context) => _WorktreeReviewDialog(
          branch: worktree.branch,
          status: status,
          stagedDiff: _readString(diff['staged']),
          unstagedDiff: _readString(diff['unstaged']),
        ),
      );
    } on Exception {
      if (mounted) _showError(l10n.projectWorktreeCheckFailed);
    }
  }

  Future<void> _chooseAndRunTask(_ProjectWorktree worktree) async {
    if (_runningTaskWorktreeId != null) return;
    final l10n = context.openchatL10n;
    try {
      final response = await widget.serviceClient.call(
        'project.worktrees.tasks',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
          'worktreeId': worktree.id,
        },
      );
      final rawTasks = response['tasks'];
      if (rawTasks is! List<Object?>) {
        throw const FormatException('The named task list was invalid.');
      }
      final tasks = rawTasks
          .map(_ProjectTask.fromObject)
          .toList(growable: false);
      if (!mounted) return;
      if (tasks.isEmpty) {
        _showError(l10n.projectWorktreeTaskEmpty);
        return;
      }
      final task = await showDialog<_ProjectTask>(
        context: context,
        builder: (context) => _ProjectTaskPickerDialog(tasks: tasks),
      );
      if (task == null || !mounted) return;
      final taskPermissionRule =
          widget.projectPermissionRules['run_project_task__${task.id}'] ??
          widget.projectPermissionRules['run_project_task'];
      if (taskPermissionRule == ToolPermissionRule.deny) {
        _showError(l10n.projectWorktreeTaskDenied);
        return;
      }
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l10n.projectWorktreeTaskConfirmationTitle),
          content: SizedBox(
            width: double.maxFinite,
            height: math.min(280, MediaQuery.sizeOf(context).height * 0.38),
            child: ListView(
              children: [
                Text(l10n.projectWorktreeBranch(worktree.branch)),
                const SizedBox(height: 12),
                Text(l10n.projectWorktreeTaskCommand),
                const SizedBox(height: 4),
                SelectableText(task.command),
                const SizedBox(height: 12),
                Text(l10n.projectWorktreeTaskTimeout(task.timeoutSeconds)),
              ],
            ),
          ),
          actions: [
            TextButton(
              key: const ValueKey<String>('project-worktree-task-cancel'),
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              key: const ValueKey<String>('project-worktree-task-confirm'),
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.projectWorktreeTaskRun),
            ),
          ],
        ),
      );
      if (confirmed == true && mounted) {
        await _runProjectTask(worktree, task);
      }
    } on OpenChatServiceException catch (error) {
      if (mounted) {
        _showError(
          error.code == 'project_task_catalog_unavailable' ||
                  error.code == 'invalid_project_task_catalog'
              ? l10n.projectWorktreeTaskLoadFailed
              : l10n.projectWorktreeOperationFailed,
        );
      }
    } on Exception {
      if (mounted) _showError(l10n.projectWorktreeTaskLoadFailed);
    }
  }

  Future<void> _runProjectTask(
    _ProjectWorktree worktree,
    _ProjectTask task,
  ) async {
    final l10n = context.openchatL10n;
    setState(() {
      _runningTaskWorktreeId = worktree.id;
      _isCancellingTask = false;
    });
    try {
      final operation = await widget.serviceClient.startOperation(
        'project.worktrees.run_task',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
          'worktreeId': worktree.id,
          'taskId': task.id,
          'expectedCommand': task.command,
          'expectedTimeoutSeconds': task.timeoutSeconds,
          'toolPermissionMode': widget.permissionMode.serviceValue,
          'confirmed': true,
          'toolPermissionRules': Map<String, Object?>.fromEntries(
            widget.projectPermissionRules.entries
                .where((entry) => entry.value != ToolPermissionRule.inherit)
                .map(
                  (entry) => MapEntry<String, Object?>(
                    entry.key,
                    entry.value.serviceValue,
                  ),
                ),
          ),
        },
      );
      if (!mounted) {
        await operation.cancel();
        return;
      }
      setState(() => _runningTaskOperation = operation);
      final result = await operation.result;
      if (!mounted) return;
      final status = _requiredString(result, 'status');
      final exitCodeValue = result['exitCode'];
      if (exitCodeValue != null && exitCodeValue is! int) {
        throw const FormatException('The named task exit code was invalid.');
      }
      final exitCode = exitCodeValue is int ? exitCodeValue : null;
      final output = _readString(result['output']);
      final truncatedValue = result['truncated'];
      if (truncatedValue is! bool) {
        throw const FormatException('The named task output state was invalid.');
      }
      final truncated = truncatedValue;
      final statusLabel = switch (status) {
        'completed' when exitCode != null => l10n.projectWorktreeTaskExitCode(
          exitCode,
        ),
        'completed' => l10n.projectWorktreeTaskExitCodeUnavailable,
        'timed_out' => l10n.projectWorktreeTaskTimedOut,
        'cancelled' => l10n.projectWorktreeTaskCancelled,
        _ => l10n.projectWorktreeTaskRunFailed,
      };
      await showDialog<void>(
        context: context,
        builder: (context) => _ProjectTaskResultDialog(
          taskId: task.id,
          status: statusLabel,
          output: output,
          truncated: truncated,
        ),
      );
    } on OpenChatServiceException catch (error) {
      if (mounted) {
        final message = switch (error.code) {
          'operation_cancelled' when _isCancellingTask =>
            l10n.projectWorktreeTaskCancelled,
          'project_task_changed' => l10n.projectWorktreeTaskLoadFailed,
          'project_task_execution_failed' => l10n.projectWorktreeTaskRunFailed,
          'permission_denied' => l10n.projectWorktreeTaskDenied,
          _ => l10n.projectWorktreeOperationFailed,
        };
        _showError(message);
      }
    } on Exception {
      if (mounted) _showError(l10n.projectWorktreeTaskRunFailed);
    } finally {
      if (mounted) {
        setState(() {
          _runningTaskWorktreeId = null;
          _runningTaskOperation = null;
          _isCancellingTask = false;
        });
      }
    }
  }

  Future<void> _cancelProjectTask() async {
    final operation = _runningTaskOperation;
    if (operation == null || _isCancellingTask) return;
    setState(() => _isCancellingTask = true);
    await operation.cancel();
  }

  Future<void> _removeWorktree(_ProjectWorktree worktree) async {
    final l10n = context.openchatL10n;
    final remove = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.projectWorktreeRemoveTitle),
        content: Text(l10n.projectWorktreeRemoveDescription(worktree.branch)),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton.tonal(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.projectWorktreeRemove),
          ),
        ],
      ),
    );
    if (remove != true || !mounted) return;
    setState(() {
      _removingWorktreeId = worktree.id;
      _errorCode = null;
    });
    try {
      await widget.serviceClient.call(
        'project.worktrees.remove',
        params: <String, Object?>{
          'projectId': widget.projectId,
          'projectRoot': widget.projectRoot,
          'worktreeId': worktree.id,
        },
        timeout: const Duration(seconds: 50),
      );
      await _loadWorktrees();
    } on Exception {
      if (mounted) setState(() => _errorCode = 'unavailable');
    } finally {
      if (mounted) setState(() => _removingWorktreeId = null);
    }
  }

  void _showError(String message) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  String? _localizedError() {
    final l10n = context.openchatL10n;
    return switch (_errorCode) {
      null => null,
      'project_not_git_repository' => l10n.projectWorktreeNotRepository,
      _ => l10n.projectWorktreeOperationFailed,
    };
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return AlertDialog(
      title: Text(l10n.projectWorktreesTitle),
      content: SizedBox(
        width: double.maxFinite,
        height: math.min(500, MediaQuery.sizeOf(context).height * 0.62),
        child: ListView(
          children: [
            Text(
              l10n.projectWorktreesDescription,
              style: TextStyle(color: palette.secondaryText),
            ),
            const SizedBox(height: 12),
            if (_localizedError() case final error? when _worktrees.isNotEmpty)
              Semantics(liveRegion: true, child: Text(error)),
            if (_isLoading)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 28),
                child: Center(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const CircularProgressIndicator(),
                      const SizedBox(height: 12),
                      Text(l10n.projectWorktreesLoading),
                    ],
                  ),
                ),
              )
            else if (_worktrees.isEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 28),
                child: Center(
                  child: Text(
                    _errorCode == null
                        ? l10n.projectWorktreesEmpty
                        : _localizedError() ??
                              l10n.projectWorktreeOperationFailed,
                    textAlign: TextAlign.center,
                    style: TextStyle(color: palette.secondaryText),
                  ),
                ),
              )
            else
              for (final worktree in _worktrees)
                _buildWorktreeCard(context, worktree, l10n, palette),
            if (_truncated)
              Padding(
                padding: const EdgeInsets.all(8),
                child: Text(
                  l10n.projectWorktreeListTruncated,
                  style: TextStyle(color: palette.secondaryText),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _isCreating || _isUsingWorktree
              ? null
              : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton.icon(
          onPressed: _isLoading || _isCreating || _isUsingWorktree
              ? null
              : _createWorktree,
          icon: _isCreating
              ? const SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(LucideIcons.gitBranch),
          label: Text(l10n.projectWorktreeCreate),
        ),
      ],
    );
  }

  Widget _buildWorktreeCard(
    BuildContext context,
    _ProjectWorktree worktree,
    AppLocalizations l10n,
    OpenChatPalette palette,
  ) {
    final busy =
        _isUsingWorktree ||
        _removingWorktreeId == worktree.id ||
        (_runningTaskWorktreeId != null &&
            _runningTaskWorktreeId != worktree.id);
    return Card(
      key: ValueKey<String>(worktree.id),
      margin: const EdgeInsets.only(bottom: 8),
      child: Padding(
        padding: const EdgeInsets.all(10),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.projectWorktreeBranch(worktree.branch),
              style: Theme.of(context).textTheme.titleSmall,
            ),
            const SizedBox(height: 4),
            Text(
              l10n.projectWorktreePath(worktree.path),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            ),
            const SizedBox(height: 4),
            Text(
              worktree.changedFileCount == 0
                  ? l10n.projectWorktreeStatusClean
                  : l10n.projectWorktreeStatusChanges(
                      worktree.changedFileCount,
                    ),
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            ),
            Wrap(
              spacing: 4,
              runSpacing: 4,
              children: [
                TextButton.icon(
                  onPressed: busy ? null : () => _reviewWorktree(worktree),
                  icon: const Icon(LucideIcons.gitCompare),
                  label: Text(l10n.projectWorktreeReview),
                ),
                TextButton.icon(
                  onPressed: _runningTaskWorktreeId == worktree.id
                      ? _isCancellingTask
                            ? null
                            : _cancelProjectTask
                      : busy
                      ? null
                      : () => _chooseAndRunTask(worktree),
                  icon: _runningTaskWorktreeId == worktree.id
                      ? const SizedBox.square(
                          dimension: 14,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(LucideIcons.terminal),
                  label: Text(
                    _runningTaskWorktreeId == worktree.id
                        ? _isCancellingTask
                              ? l10n.projectWorktreeTaskStopping
                              : l10n.projectWorktreeTaskStop
                        : l10n.projectWorktreeRunCheck,
                  ),
                ),
                TextButton.icon(
                  onPressed: busy ? null : () => _useWorktree(worktree),
                  icon: const Icon(LucideIcons.folderOpen),
                  label: Text(l10n.projectWorktreeUse),
                ),
                TextButton.icon(
                  onPressed: busy ? null : () => _removeWorktree(worktree),
                  icon: const Icon(LucideIcons.trash2),
                  label: Text(l10n.projectWorktreeRemove),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _WorktreeReviewDialog extends StatelessWidget {
  const _WorktreeReviewDialog({
    required this.branch,
    required this.status,
    required this.stagedDiff,
    required this.unstagedDiff,
  });

  final String branch;
  final Map<String, Object?> status;
  final String stagedDiff;
  final String unstagedDiff;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final files = status['files'];
    final fileCount = files is List<Object?> ? files.length : 0;
    return AlertDialog(
      title: Text(l10n.projectWorktreeReviewTitle),
      content: SizedBox(
        width: double.maxFinite,
        height: math.min(520, MediaQuery.sizeOf(context).height * 0.64),
        child: fileCount == 0 && stagedDiff.isEmpty && unstagedDiff.isEmpty
            ? Center(child: Text(l10n.projectWorktreeNoChanges))
            : ListView(
                children: [
                  Text(l10n.projectWorktreeBranch(branch)),
                  if (stagedDiff.isNotEmpty) ...[
                    const SizedBox(height: 12),
                    Text(l10n.projectWorktreeStagedDiff),
                    const SizedBox(height: 4),
                    SelectableText(stagedDiff),
                  ],
                  if (unstagedDiff.isNotEmpty) ...[
                    const SizedBox(height: 12),
                    Text(l10n.projectWorktreeUnstagedDiff),
                    const SizedBox(height: 4),
                    SelectableText(unstagedDiff),
                  ],
                  if (fileCount > 0 &&
                      stagedDiff.isEmpty &&
                      unstagedDiff.isEmpty)
                    Text(l10n.projectWorktreeStatusChanges(fileCount)),
                ],
              ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    );
  }
}

class _ProjectWorktree {
  const _ProjectWorktree({
    required this.id,
    required this.branch,
    required this.path,
    required this.changedFileCount,
  });

  final String id;
  final String branch;
  final String path;
  final int changedFileCount;

  factory _ProjectWorktree.fromObject(Object? value) {
    final data = _readMap(value);
    final statusValue = data['status'];
    final status = statusValue == null
        ? const <String, Object?>{}
        : _readMap(statusValue);
    final files = status['files'];
    if (files is! List<Object?>) {
      return _ProjectWorktree(
        id: _requiredString(data, 'id'),
        branch: _requiredString(data, 'branch'),
        path: _requiredString(data, 'path'),
        changedFileCount: 0,
      );
    }
    return _ProjectWorktree(
      id: _requiredString(data, 'id'),
      branch: _requiredString(data, 'branch'),
      path: _requiredString(data, 'path'),
      changedFileCount: files.length,
    );
  }
}

class _ProjectTask {
  const _ProjectTask({
    required this.id,
    required this.command,
    required this.timeoutSeconds,
  });

  final String id;
  final String command;
  final int timeoutSeconds;

  factory _ProjectTask.fromObject(Object? value) {
    final data = _readMap(value);
    final id = _requiredString(data, 'id');
    final command = _requiredString(data, 'command');
    final timeoutSeconds = data['timeoutSeconds'];
    if (timeoutSeconds is! int || timeoutSeconds < 5 || timeoutSeconds > 600) {
      throw const FormatException('A named project task was invalid.');
    }
    return _ProjectTask(
      id: id,
      command: command,
      timeoutSeconds: timeoutSeconds,
    );
  }
}

class _ProjectTaskPickerDialog extends StatefulWidget {
  const _ProjectTaskPickerDialog({required this.tasks});

  final List<_ProjectTask> tasks;

  @override
  State<_ProjectTaskPickerDialog> createState() =>
      _ProjectTaskPickerDialogState();
}

class _ProjectTaskPickerDialogState extends State<_ProjectTaskPickerDialog> {
  late String _selectedTaskId = widget.tasks.first.id;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.projectWorktreeTaskPickerTitle),
      content: SizedBox(
        width: double.maxFinite,
        height: math.min(400, MediaQuery.sizeOf(context).height * 0.5),
        child: RadioGroup<String>(
          groupValue: _selectedTaskId,
          onChanged: (value) {
            if (value != null) setState(() => _selectedTaskId = value);
          },
          child: ListView.builder(
            itemCount: widget.tasks.length,
            itemBuilder: (context, index) {
              final task = widget.tasks[index];
              final selected = task.id == _selectedTaskId;
              return Semantics(
                selected: selected,
                child: ListTile(
                  key: ValueKey<String>('project-task-${task.id}'),
                  leading: Radio<String>(value: task.id),
                  title: Text(task.id),
                  subtitle: Text(
                    task.command,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  selected: selected,
                  onTap: () => setState(() => _selectedTaskId = task.id),
                ),
              );
            },
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          key: const ValueKey<String>('project-worktree-task-run'),
          onPressed: () => Navigator.of(
            context,
          ).pop(widget.tasks.firstWhere((task) => task.id == _selectedTaskId)),
          child: Text(l10n.projectWorktreeTaskRun),
        ),
      ],
    );
  }
}

class _ProjectTaskResultDialog extends StatelessWidget {
  const _ProjectTaskResultDialog({
    required this.taskId,
    required this.status,
    required this.output,
    required this.truncated,
  });

  final String taskId;
  final String status;
  final String output;
  final bool truncated;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return AlertDialog(
      title: Text(l10n.projectWorktreeTaskResultTitle(taskId)),
      content: SizedBox(
        width: double.maxFinite,
        height: math.min(400, MediaQuery.sizeOf(context).height * 0.5),
        child: ListView(
          children: [
            Semantics(liveRegion: true, child: Text(status)),
            if (truncated) ...[
              const SizedBox(height: 8),
              Text(l10n.projectWorktreeTaskOutputTruncated),
            ],
            const SizedBox(height: 12),
            Text(l10n.projectWorktreeTaskOutput),
            const SizedBox(height: 4),
            if (output.isEmpty)
              Text(l10n.projectWorktreeTaskNoOutput)
            else
              SelectableText(output),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    );
  }
}

Map<String, Object?> _readMap(Object? value) {
  if (value is! Map) {
    throw const FormatException('A project worktree response was invalid.');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw const FormatException('A project worktree response was invalid.');
    }
    result[entry.key as String] = entry.value;
  }
  return result;
}

String _requiredString(Map<String, Object?> data, String key) {
  final value = data[key];
  if (value is! String || value.isEmpty) {
    throw const FormatException('A project worktree response was invalid.');
  }
  return value;
}

String _readString(Object? value) => value is String ? value : '';
