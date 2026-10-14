import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ToolPermissionsSettingsSection extends StatefulWidget {
  const ToolPermissionsSettingsSection({
    required this.settingsPreferences,
    this.projectId,
    this.onOpenProjectPermissions,
    this.onSaved,
    super.key,
  });

  final SettingsPreferences settingsPreferences;
  final String? projectId;
  final VoidCallback? onOpenProjectPermissions;
  final Future<void> Function()? onSaved;

  @override
  State<ToolPermissionsSettingsSection> createState() =>
      _ToolPermissionsSettingsSectionState();
}

class _ToolPermissionsSettingsSectionState
    extends State<ToolPermissionsSettingsSection> {
  ToolPermissionMode? _mode;
  bool _isLoading = true;
  bool _isSaving = false;
  bool _loadFailed = false;
  Map<String, ToolPermissionRule>? _projectRules;
  bool _isLoadingProjectRules = false;
  bool _projectRulesLoadFailed = false;
  int _projectRulesGeneration = 0;

  @override
  void initState() {
    super.initState();
    unawaited(_loadMode());
    unawaited(_loadProjectRules());
  }

  @override
  void didUpdateWidget(covariant ToolPermissionsSettingsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.projectId != widget.projectId ||
        oldWidget.settingsPreferences != widget.settingsPreferences) {
      unawaited(_loadProjectRules());
    }
  }

  Future<void> _loadProjectRules() async {
    final generation = ++_projectRulesGeneration;
    final projectId = widget.projectId;
    if (projectId == null) {
      if (mounted) {
        setState(() {
          _projectRules = null;
          _isLoadingProjectRules = false;
          _projectRulesLoadFailed = false;
        });
      }
      return;
    }
    setState(() {
      _isLoadingProjectRules = true;
      _projectRulesLoadFailed = false;
    });
    try {
      final rules = await widget.settingsPreferences
          .readProjectToolPermissionRules(projectId);
      if (!mounted || generation != _projectRulesGeneration) return;
      setState(() {
        _projectRules = rules;
        _isLoadingProjectRules = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'tool_permissions',
          context: ErrorDescription('while loading project tool permissions'),
        ),
      );
      if (!mounted || generation != _projectRulesGeneration) return;
      setState(() {
        _isLoadingProjectRules = false;
        _projectRulesLoadFailed = true;
      });
    }
  }

  Future<void> _loadMode() async {
    setState(() {
      _isLoading = true;
      _loadFailed = false;
    });
    try {
      final mode = await widget.settingsPreferences.readToolPermissionMode();
      if (!mounted) return;
      setState(() {
        _mode = mode;
        _isLoading = false;
      });
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'tool_permissions',
          context: ErrorDescription('while loading global tool permissions'),
        ),
      );
      if (!mounted) return;
      setState(() {
        _isLoading = false;
        _loadFailed = true;
      });
    }
  }

  Future<void> _saveMode(ToolPermissionMode mode) async {
    if (_isSaving || mode == _mode || mode == ToolPermissionMode.plan) return;
    setState(() => _isSaving = true);
    try {
      await widget.settingsPreferences.writeToolPermissionMode(mode);
      if (!mounted) return;
      setState(() => _mode = mode);
      await widget.onSaved?.call();
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'tool_permissions',
          context: ErrorDescription('while saving global tool permissions'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          context.openchatL10n.toolPermissionSettingsSaveFailed,
          type: OpenChatToastType.error,
        );
      }
    } finally {
      if (mounted) setState(() => _isSaving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final mode = _mode;

    if (_isLoading) {
      return const Center(
        child: SizedBox.square(
          dimension: 22,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      );
    }
    if (_loadFailed || mode == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.toolPermissionSettingsLoadFailed,
            style: TextStyle(color: palette.secondaryText),
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: () => unawaited(_loadMode()),
            icon: const Icon(LucideIcons.refreshCw, size: 16),
            label: Text(l10n.retry),
          ),
        ],
      );
    }

    final options =
        <(ToolPermissionMode mode, String title, String description)>[
          (
            ToolPermissionMode.requireApproval,
            l10n.toolPermissionRequireApproval,
            l10n.toolPermissionRequireApprovalDescription,
          ),
          (
            ToolPermissionMode.approveSafeOperations,
            l10n.toolPermissionApproveSafeOperations,
            l10n.toolPermissionApproveSafeOperationsDescription,
          ),
          (
            ToolPermissionMode.fullAccess,
            l10n.toolPermissionFullAccess,
            l10n.toolPermissionFullAccessDescription,
          ),
        ];

    return RadioGroup<ToolPermissionMode>(
      groupValue: mode,
      onChanged: (value) {
        if (!_isSaving && value != null) unawaited(_saveMode(value));
      },
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final option in options)
            RadioListTile<ToolPermissionMode>(
              contentPadding: EdgeInsets.zero,
              value: option.$1,
              title: Text(option.$2),
              subtitle: Text(option.$3),
            ),
          if (widget.onOpenProjectPermissions != null) ...[
            const Divider(height: 24),
            Text(
              l10n.projectToolRulesDescription,
              style: TextStyle(color: palette.secondaryText),
            ),
            if (widget.projectId != null) ...[
              const SizedBox(height: 8),
              _buildProjectRulesSummary(context),
            ],
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                onPressed: _isSaving ? null : widget.onOpenProjectPermissions,
                icon: const Icon(LucideIcons.slidersHorizontal, size: 16),
                label: Text(l10n.projectToolRulesTitle),
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildProjectRulesSummary(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    if (_isLoadingProjectRules) {
      return Semantics(
        label: l10n.projectToolRulesSummaryLoading,
        child: const LinearProgressIndicator(minHeight: 2),
      );
    }
    if (_projectRulesLoadFailed || _projectRules == null) {
      return Row(
        children: [
          Expanded(
            child: Text(
              l10n.projectToolRulesSummaryUnavailable,
              style: TextStyle(color: palette.secondaryText),
            ),
          ),
          IconButton(
            tooltip: l10n.retry,
            onPressed: () => unawaited(_loadProjectRules()),
            icon: const Icon(LucideIcons.refreshCw, size: 16),
          ),
        ],
      );
    }
    final rules = _projectRules!.values;
    final allowed = rules
        .where((rule) => rule == ToolPermissionRule.allow)
        .length;
    final approval = rules
        .where((rule) => rule == ToolPermissionRule.ask)
        .length;
    final denied = rules
        .where((rule) => rule == ToolPermissionRule.deny)
        .length;
    return Text(
      rules.isEmpty
          ? l10n.projectToolRulesSummaryInherit
          : l10n.projectToolRulesSummary(allowed, approval, denied),
      style: TextStyle(color: palette.secondaryText),
    );
  }
}
