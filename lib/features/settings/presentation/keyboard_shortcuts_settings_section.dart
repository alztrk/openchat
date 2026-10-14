import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/features/settings/domain/keyboard_shortcut_bindings.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

class KeyboardShortcutsSettingsSection extends StatefulWidget {
  const KeyboardShortcutsSettingsSection({
    required this.settingsPreferences,
    this.onChanged,
    super.key,
  });

  final SettingsPreferences settingsPreferences;
  final Future<void> Function()? onChanged;

  @override
  State<KeyboardShortcutsSettingsSection> createState() =>
      _KeyboardShortcutsSettingsSectionState();
}

class _KeyboardShortcutsSettingsSectionState
    extends State<KeyboardShortcutsSettingsSection> {
  KeyboardShortcutBindings _bindings = KeyboardShortcutBindings.defaults;
  ComposerShortcutAction? _recordingAction;
  String? _error;
  bool _loading = true;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    try {
      final bindings = await widget.settingsPreferences.readKeyboardShortcuts();
      if (mounted) setState(() => _bindings = bindings);
    } catch (_) {
      if (mounted) setState(() => _error = 'load');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _save(
    ComposerShortcutAction action,
    KeyboardShortcutBinding binding,
  ) async {
    if (_bindings.conflicts(action, binding)) {
      setState(() => _error = 'conflict');
      return;
    }
    final updated = _bindings.withBinding(action, binding);
    setState(() {
      _saving = true;
      _error = null;
      _recordingAction = null;
    });
    try {
      await widget.settingsPreferences.writeKeyboardShortcuts(updated);
      if (!mounted) return;
      setState(() => _bindings = updated);
      await widget.onChanged?.call();
    } catch (_) {
      if (mounted) setState(() => _error = 'save');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Future<void> _reset() async {
    setState(() {
      _saving = true;
      _error = null;
      _recordingAction = null;
    });
    try {
      await widget.settingsPreferences.writeKeyboardShortcuts(
        KeyboardShortcutBindings.defaults,
      );
      if (!mounted) return;
      setState(() => _bindings = KeyboardShortcutBindings.defaults);
      await widget.onChanged?.call();
    } catch (_) {
      if (mounted) setState(() => _error = 'save');
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  String _actionName(AppLocalizations l10n, ComposerShortcutAction action) =>
      switch (action) {
        ComposerShortcutAction.sendMessage => l10n.keyboardSendMessage,
        ComposerShortcutAction.insertNewLine => l10n.keyboardNewLine,
        ComposerShortcutAction.stopResponse => l10n.keyboardStopResponse,
      };

  String _actionDescription(
    AppLocalizations l10n,
    ComposerShortcutAction action,
  ) => switch (action) {
    ComposerShortcutAction.sendMessage => l10n.keyboardSendMessageDescription,
    ComposerShortcutAction.insertNewLine => l10n.keyboardNewLineDescription,
    ComposerShortcutAction.stopResponse => l10n.keyboardStopResponseDescription,
  };

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_loading) return const LinearProgressIndicator();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_error != null) ...[
          Text(switch (_error) {
            'load' => l10n.keyboardShortcutLoadFailed,
            'save' => l10n.keyboardShortcutSaveFailed,
            _ => l10n.keyboardShortcutConflict,
          }, style: TextStyle(color: palette.destructive)),
          const SizedBox(height: OpenChatSpacing.sm),
        ],
        Card(
          margin: EdgeInsets.zero,
          color: palette.surface,
          elevation: 0,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(OpenChatRadii.card),
            side: BorderSide(color: palette.border),
          ),
          child: Column(
            children: [
              for (
                var index = 0;
                index < ComposerShortcutAction.values.length;
                index++
              ) ...[
                if (index > 0) Divider(height: 1, color: palette.border),
                _buildShortcutRow(
                  context,
                  ComposerShortcutAction.values[index],
                ),
              ],
            ],
          ),
        ),
        const SizedBox(height: OpenChatSpacing.sm),
        Align(
          alignment: Alignment.centerRight,
          child: TextButton.icon(
            onPressed: _saving ? null : _reset,
            icon: const Icon(LucideIcons.rotateCcw, size: 16),
            label: Text(l10n.keyboardShortcutsReset),
          ),
        ),
      ],
    );
  }

  Widget _buildShortcutRow(
    BuildContext context,
    ComposerShortcutAction action,
  ) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final recording = _recordingAction == action;
    final binding = _bindings.forAction(action);

    return Padding(
      padding: const EdgeInsets.all(OpenChatSpacing.sm),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final details = Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(_actionName(l10n, action)),
              const SizedBox(height: OpenChatSpacing.xxs),
              Text(
                _actionDescription(l10n, action),
                style: TextStyle(color: palette.secondaryText),
              ),
            ],
          );
          final control = Focus(
            autofocus: recording,
            onKeyEvent: (node, event) {
              if (!recording || event is! KeyDownEvent) {
                return KeyEventResult.ignored;
              }
              if (event.logicalKey == LogicalKeyboardKey.escape) {
                setState(() => _recordingAction = null);
                return KeyEventResult.handled;
              }
              if (event.logicalKey == LogicalKeyboardKey.controlLeft ||
                  event.logicalKey == LogicalKeyboardKey.controlRight ||
                  event.logicalKey == LogicalKeyboardKey.shiftLeft ||
                  event.logicalKey == LogicalKeyboardKey.shiftRight ||
                  event.logicalKey == LogicalKeyboardKey.altLeft ||
                  event.logicalKey == LogicalKeyboardKey.altRight ||
                  event.logicalKey == LogicalKeyboardKey.metaLeft ||
                  event.logicalKey == LogicalKeyboardKey.metaRight) {
                return KeyEventResult.handled;
              }
              unawaited(
                _save(
                  action,
                  KeyboardShortcutBinding.fromKeyEvent(
                    event,
                    HardwareKeyboard.instance,
                  ),
                ),
              );
              return KeyEventResult.handled;
            },
            child: OutlinedButton.icon(
              onPressed: _saving
                  ? null
                  : () => setState(() {
                      _error = null;
                      _recordingAction = recording ? null : action;
                    }),
              icon: Icon(
                recording ? LucideIcons.keyboard : LucideIcons.pencil,
                size: 16,
              ),
              label: Text(
                recording ? l10n.keyboardShortcutRecord : binding.displayLabel,
              ),
            ),
          );
          if (constraints.maxWidth < 520) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                details,
                const SizedBox(height: OpenChatSpacing.sm),
                control,
              ],
            );
          }
          return Row(
            children: [
              Expanded(child: details),
              const SizedBox(width: OpenChatSpacing.md),
              control,
            ],
          );
        },
      ),
    );
  }
}
