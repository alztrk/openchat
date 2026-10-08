import 'package:flutter/material.dart';

import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

class ProjectToolPermissionsDialog extends StatefulWidget {
  const ProjectToolPermissionsDialog({
    super.key,
    required this.initialRules,
    this.namedTaskIds = const <String>[],
    this.configuredToolIds = const <String>[],
    required this.onSave,
  });

  final Map<String, ToolPermissionRule> initialRules;
  final List<String> namedTaskIds;
  final List<String> configuredToolIds;
  final Future<void> Function(Map<String, ToolPermissionRule> rules) onSave;

  @override
  State<ProjectToolPermissionsDialog> createState() =>
      _ProjectToolPermissionsDialogState();
}

String _dynamicToolLabel(AppLocalizations l10n, String name) {
  const taskPrefix = 'run_project_task__';
  if (name.startsWith(taskPrefix)) {
    return '${l10n.toolRunProjectTask}: ${name.substring(taskPrefix.length)}';
  }
  const configuredPrefix = 'project_tool__';
  return l10n.toolConfiguredProjectTool(
    name.substring(configuredPrefix.length),
  );
}

class _ProjectToolPermissionsDialogState
    extends State<ProjectToolPermissionsDialog> {
  late final Map<String, ToolPermissionRule> _rules =
      Map<String, ToolPermissionRule>.fromEntries(
        widget.initialRules.entries.where((entry) {
          const prefix = 'run_project_task__';
          if (entry.key.startsWith(prefix)) {
            return widget.namedTaskIds.contains(
              entry.key.substring(prefix.length),
            );
          }
          const configuredPrefix = 'project_tool__';
          if (entry.key.startsWith(configuredPrefix)) {
            return widget.configuredToolIds.contains(
              entry.key.substring(configuredPrefix.length),
            );
          }
          return true;
        }),
      );
  bool _saving = false;
  bool _saveFailed = false;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final labels = <String, String>{
      'list_files': l10n.toolListFiles,
      'search_files': l10n.toolSearchFiles,
      'read_file': l10n.toolReadFile,
      'write_file': l10n.toolWriteFile,
      'edit_file': l10n.toolEditFile,
      'get_file_info': l10n.toolGetFileInfo,
      'execute_command': l10n.toolExecuteCommand,
      'send_terminal_input': l10n.toolSendTerminalInput,
      'git_status': l10n.toolGitStatus,
      'git_diff': l10n.toolGitDiff,
      'git_history': l10n.toolGitHistory,
      'web_search': l10n.toolWebSearch,
      'read_url_content': l10n.toolReadUrlContent,
      'delegate_task': l10n.toolDelegateTask,
      'run_project_task': l10n.toolRunProjectTask,
    };
    final ruleNames = <String>{
      ...projectToolRuleNames,
      ...widget.namedTaskIds.map((id) => 'run_project_task__$id'),
      ...widget.configuredToolIds.map((id) => 'project_tool__$id'),
    };

    return AlertDialog(
      title: Text(l10n.projectToolRulesTitle),
      content: SizedBox(
        width: 520,
        height: MediaQuery.sizeOf(context).height * 0.62,
        child: ListView(
          children: [
            Text(l10n.projectToolRulesDescription),
            const SizedBox(height: 16),
            for (final name in ruleNames)
              Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: DropdownButtonFormField<ToolPermissionRule>(
                  key: ValueKey<String>('project-tool-rule-$name'),
                  initialValue: _rules[name] ?? ToolPermissionRule.inherit,
                  isExpanded: true,
                  decoration: InputDecoration(
                    labelText: labels[name] ?? _dynamicToolLabel(l10n, name),
                  ),
                  items: [
                    DropdownMenuItem(
                      value: ToolPermissionRule.inherit,
                      child: Text(l10n.projectToolRuleInherit),
                    ),
                    DropdownMenuItem(
                      value: ToolPermissionRule.ask,
                      child: Text(l10n.projectToolRuleAsk),
                    ),
                    DropdownMenuItem(
                      value: ToolPermissionRule.allow,
                      child: Text(l10n.projectToolRuleAllow),
                    ),
                    DropdownMenuItem(
                      value: ToolPermissionRule.deny,
                      child: Text(l10n.projectToolRuleDeny),
                    ),
                  ],
                  onChanged: _saving
                      ? null
                      : (value) {
                          if (value == null) return;
                          setState(() {
                            if (value == ToolPermissionRule.inherit) {
                              _rules.remove(name);
                            } else {
                              _rules[name] = value;
                            }
                            _saveFailed = false;
                          });
                        },
                ),
              ),
            if (_saveFailed) ...[
              const SizedBox(height: 8),
              Text(
                l10n.projectToolRulesSaveFailed,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: _saving
              ? const SizedBox.square(
                  dimension: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.save),
        ),
      ],
    );
  }

  Future<void> _save() async {
    setState(() {
      _saving = true;
      _saveFailed = false;
    });
    try {
      await widget.onSave(Map<String, ToolPermissionRule>.unmodifiable(_rules));
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (mounted) {
        setState(() {
          _saving = false;
          _saveFailed = true;
        });
      }
    }
  }
}
