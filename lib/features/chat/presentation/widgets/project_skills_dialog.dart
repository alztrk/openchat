import 'package:flutter/material.dart';

import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

class ProjectSkillsDialog extends StatefulWidget {
  const ProjectSkillsDialog({
    required this.serviceClient,
    required this.settingsPreferences,
    required this.projectId,
    super.key,
  });

  final OpenChatServiceClient serviceClient;
  final SettingsPreferences settingsPreferences;
  final String projectId;

  @override
  State<ProjectSkillsDialog> createState() => _ProjectSkillsDialogState();
}

class _ProjectSkillsDialogState extends State<ProjectSkillsDialog> {
  List<_ProjectSkill> _skills = const <_ProjectSkill>[];
  Set<String> _selectedIds = <String>{};
  bool _isLoading = true;
  bool _isSaving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _isLoading = true;
      _error = null;
    });
    try {
      final response = await widget.serviceClient.call(
        'project.skills.list',
        params: <String, Object?>{'projectId': widget.projectId},
      );
      final rawSkills = response['skills'];
      if (rawSkills is! List<Object?> || rawSkills.length > 32) {
        throw const FormatException('Project Skills were invalid.');
      }
      final skills = rawSkills
          .map(_ProjectSkill.fromJson)
          .toList(growable: false);
      final ids = skills.map((skill) => skill.id).toSet();
      if (ids.length != skills.length) {
        throw const FormatException('Project Skill IDs were not unique.');
      }
      final savedIds = await widget.settingsPreferences.readProjectSkillIds(
        widget.projectId,
      );
      if (!mounted) return;
      setState(() {
        _skills = skills;
        _selectedIds = Set<String>.of(savedIds);
        _isLoading = false;
      });
    } on Exception {
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _error = context.openchatL10n.projectSkillsLoadFailed;
      });
    }
  }

  Future<void> _save() async {
    if (_isSaving || _isLoading || _error != null) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writeProjectSkillIds(
        widget.projectId,
        _selectedIds,
      );
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (!mounted) return;
      setState(() {
        _isSaving = false;
        _error = context.openchatL10n.projectSkillsSaveFailed;
      });
    }
  }

  Future<void> _clearSelection() async {
    if (_isSaving) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writeProjectSkillIds(
        widget.projectId,
        <String>{},
      );
      if (mounted) Navigator.of(context).pop();
    } on Exception {
      if (!mounted) return;
      setState(() {
        _isSaving = false;
        _error = context.openchatL10n.projectSkillsSaveFailed;
      });
    }
  }

  void _toggle(String id, bool selected) {
    setState(() {
      if (selected) {
        _selectedIds.add(id);
      } else {
        _selectedIds.remove(id);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final error = _error;
    final availableIds = _skills.map((skill) => skill.id).toSet();
    final staleIds = _selectedIds.difference(availableIds).toList()..sort();
    return AlertDialog(
      title: Text(l10n.projectSkillsTitle),
      content: SizedBox(
        width: 560,
        child: _isLoading
            ? const SizedBox(
                height: 200,
                child: Center(child: CircularProgressIndicator()),
              )
            : error != null
            ? Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(error),
                  TextButton(
                    onPressed: _load,
                    child: Text(l10n.agentRunRefresh),
                  ),
                ],
              )
            : _skills.isEmpty && staleIds.isEmpty
            ? Text(l10n.projectSkillsEmpty)
            : ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: MediaQuery.sizeOf(context).height * 0.65,
                ),
                child: ListView(
                  shrinkWrap: true,
                  children: [
                    Text(l10n.projectSkillsDescription),
                    for (final skill in _skills)
                      ExpansionTile(
                        key: ValueKey(skill.id),
                        title: Text(skill.id),
                        leading: Checkbox(
                          value: _selectedIds.contains(skill.id),
                          onChanged: _isSaving
                              ? null
                              : (selected) =>
                                    _toggle(skill.id, selected ?? false),
                        ),
                        children: [
                          Padding(
                            padding: const EdgeInsetsDirectional.fromSTEB(
                              16,
                              0,
                              16,
                              16,
                            ),
                            child: SelectableText(skill.content),
                          ),
                        ],
                      ),
                    for (final id in staleIds)
                      CheckboxListTile(
                        value: true,
                        onChanged: _isSaving
                            ? null
                            : (selected) => _toggle(id, selected ?? false),
                        title: Text(id),
                        subtitle: Text(l10n.projectSkillsStale),
                        controlAffinity: ListTileControlAffinity.leading,
                      ),
                  ],
                ),
              ),
      ),
      actions: [
        if (error != null)
          TextButton(
            onPressed: _isSaving ? null : _clearSelection,
            child: Text(l10n.projectSkillsClearSelection),
          ),
        TextButton(
          onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _isLoading || _isSaving || _error != null ? null : _save,
          child: _isSaving
              ? const SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.projectInstructionsSave),
        ),
      ],
    );
  }
}

class _ProjectSkill {
  const _ProjectSkill({required this.id, required this.content});

  final String id;
  final String content;

  factory _ProjectSkill.fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('Project Skill entry was invalid.');
    }
    final id = value['id'];
    final content = value['content'];
    if (id is! String ||
        !RegExp(r'^[a-z][a-z0-9-]{0,63}$').hasMatch(id) ||
        content is! String ||
        content.isEmpty ||
        content.length > 16 * 1024) {
      throw const FormatException('Project Skill entry was invalid.');
    }
    return _ProjectSkill(id: id, content: content);
  }
}
