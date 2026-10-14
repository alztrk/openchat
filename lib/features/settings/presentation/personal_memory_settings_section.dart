import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/settings/data/personal_memory_fact.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class PersonalMemorySettingsSection extends StatefulWidget {
  const PersonalMemorySettingsSection({
    required this.settingsPreferences,
    super.key,
  });

  final SettingsPreferences settingsPreferences;

  @override
  State<PersonalMemorySettingsSection> createState() =>
      _PersonalMemorySettingsSectionState();
}

class _PersonalMemorySettingsSectionState
    extends State<PersonalMemorySettingsSection> {
  List<PersonalMemoryFact> _facts = const <PersonalMemoryFact>[];
  bool _enabled = false;
  bool _isLoading = true;
  bool _isSaving = false;
  bool _loadFailed = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
    });
    try {
      final enabled = await widget.settingsPreferences
          .readPersonalMemoryEnabled();
      final facts = await widget.settingsPreferences.readPersonalMemoryFacts();
      if (!mounted) return;
      setState(() {
        _enabled = enabled;
        _facts = facts;
        _isLoading = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'personal_memory',
          context: ErrorDescription('while loading personal memories'),
        ),
      );
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _loadFailed = true;
      });
    }
  }

  Future<void> _setEnabled(bool enabled) async {
    if (_isSaving || enabled == _enabled) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writePersonalMemoryEnabled(enabled);
      if (mounted) setState(() => _enabled = enabled);
    } on Object catch (error, stackTrace) {
      _reportSaveFailure(error, stackTrace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _clearUnreadableMemory() async {
    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.personalMemoryClearTitle),
        content: Text(l10n.personalMemoryClearConfirmation),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.personalMemoryClearConfirm),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.clearPersonalMemory();
      if (!mounted) return;
      setState(() {
        _enabled = false;
        _facts = const <PersonalMemoryFact>[];
        _loadFailed = false;
      });
    } on Object catch (error, stackTrace) {
      _reportSaveFailure(
        error,
        stackTrace,
        message: l10n.personalMemoryClearFailed,
      );
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  Future<void> _editFact([PersonalMemoryFact? existing]) async {
    final l10n = context.openchatL10n;
    final content = await showDialog<String>(
      context: context,
      builder: (dialogContext) =>
          _PersonalMemoryFactDialog(initialContent: existing?.content),
    );
    if (content == null || !mounted) return;
    final normalized = content.trim();
    if (normalized.isEmpty) return;
    final updated = List<PersonalMemoryFact>.of(_facts);
    final index = existing == null
        ? -1
        : updated.indexWhere((fact) => fact.id == existing.id);
    if (existing == null) {
      if (updated.length >= maxPersonalMemoryFacts) {
        showOpenChatToast(
          context,
          l10n.personalMemoryLimitReached,
          type: OpenChatToastType.error,
        );
        return;
      }
      updated.add(PersonalMemoryFact.create(normalized));
    } else if (index >= 0) {
      updated[index] = PersonalMemoryFact.create(normalized);
    }
    await _saveFacts(updated);
  }

  Future<void> _removeFact(PersonalMemoryFact fact) async {
    final l10n = context.openchatL10n;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.personalMemoryDeleteTitle),
        content: Text(l10n.personalMemoryDeleteConfirmation),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.personalMemoryDeleteConfirm),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    final updated = _facts.where((item) => item.id != fact.id).toList();
    await _saveFacts(updated);
  }

  Future<void> _saveFacts(List<PersonalMemoryFact> updated) async {
    if (_isSaving) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writePersonalMemoryFacts(updated);
      if (mounted) setState(() => _facts = List.unmodifiable(updated));
    } on Object catch (error, stackTrace) {
      _reportSaveFailure(error, stackTrace);
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  void _reportSaveFailure(
    Object error,
    StackTrace stackTrace, {
    String? message,
  }) {
    FlutterError.reportError(
      FlutterErrorDetails(
        exception: error,
        stack: stackTrace,
        library: 'personal_memory',
        context: ErrorDescription('while saving personal memories'),
      ),
    );
    if (mounted) {
      showOpenChatToast(
        context,
        message ?? context.openchatL10n.personalMemorySaveFailed,
        type: OpenChatToastType.error,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_isLoading) {
      return const Center(
        child: SizedBox.square(
          dimension: 22,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (_loadFailed) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(l10n.personalMemoryLoadFailed),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            children: [
              OutlinedButton.icon(
                onPressed: _isSaving ? null : () => unawaited(_load()),
                icon: const Icon(LucideIcons.refreshCw, size: 16),
                label: Text(l10n.retry),
              ),
              TextButton.icon(
                onPressed: _isSaving
                    ? null
                    : () => unawaited(_clearUnreadableMemory()),
                icon: const Icon(LucideIcons.trash2, size: 16),
                label: Text(l10n.personalMemoryClearAction),
              ),
            ],
          ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.personalMemoryDescription),
        const SizedBox(height: 8),
        Text(
          l10n.personalMemoryPrivacyNotice,
          style: TextStyle(color: palette.secondaryText),
        ),
        const SizedBox(height: 8),
        SwitchListTile.adaptive(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.personalMemoryEnabled),
          subtitle: Text(l10n.personalMemoryEnabledDescription),
          value: _enabled,
          onChanged: _isSaving ? null : _setEnabled,
        ),
        Row(
          children: [
            Expanded(child: Text(l10n.personalMemoryFactsTitle)),
            FilledButton.tonalIcon(
              onPressed: _isSaving || _facts.length >= maxPersonalMemoryFacts
                  ? null
                  : () => unawaited(_editFact()),
              icon: const Icon(LucideIcons.plus, size: 16),
              label: Text(l10n.personalMemoryAdd),
            ),
          ],
        ),
        if (_facts.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12),
            child: Text(
              l10n.personalMemoryEmpty,
              style: TextStyle(color: palette.secondaryText),
            ),
          )
        else
          for (final fact in _facts)
            ListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(fact.content),
              trailing: Wrap(
                spacing: 4,
                children: [
                  IconButton(
                    tooltip: l10n.personalMemoryEdit,
                    onPressed: _isSaving
                        ? null
                        : () => unawaited(_editFact(fact)),
                    icon: const Icon(LucideIcons.pencil, size: 16),
                  ),
                  IconButton(
                    tooltip: l10n.personalMemoryDelete,
                    onPressed: _isSaving
                        ? null
                        : () => unawaited(_removeFact(fact)),
                    icon: const Icon(LucideIcons.trash2, size: 16),
                  ),
                ],
              ),
            ),
      ],
    );
  }
}

class _PersonalMemoryFactDialog extends StatefulWidget {
  const _PersonalMemoryFactDialog({this.initialContent});

  final String? initialContent;

  @override
  State<_PersonalMemoryFactDialog> createState() =>
      _PersonalMemoryFactDialogState();
}

class _PersonalMemoryFactDialogState extends State<_PersonalMemoryFactDialog> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.initialContent ?? '',
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final isEditing = widget.initialContent != null;
    return AlertDialog(
      title: Text(isEditing ? l10n.personalMemoryEdit : l10n.personalMemoryAdd),
      content: TextField(
        controller: _controller,
        autofocus: true,
        maxLength: maxPersonalMemoryFactCharacters,
        maxLines: 3,
        minLines: 1,
        textCapitalization: TextCapitalization.sentences,
        decoration: InputDecoration(
          labelText: l10n.personalMemoryFactLabel,
          hintText: l10n.personalMemoryFactHint,
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_controller.text),
          child: Text(l10n.save),
        ),
      ],
    );
  }
}
