import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ProjectToolPermissionSelect extends StatelessWidget {
  const ProjectToolPermissionSelect({
    required this.label,
    required this.value,
    required this.onChanged,
    super.key,
  });

  final String label;
  final ToolPermissionRule value;
  final ValueChanged<ToolPermissionRule>? onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    return OpenChatSelectField<ToolPermissionRule>(
      label: label,
      value: value,
      onChanged: onChanged,
      palette: OpenChatPalette.of(context),
      options: [
        OpenChatSelectOption<ToolPermissionRule>(
          value: ToolPermissionRule.inherit,
          label: l10n.projectToolRuleInherit,
        ),
        OpenChatSelectOption<ToolPermissionRule>(
          value: ToolPermissionRule.ask,
          label: l10n.projectToolRuleAsk,
        ),
        OpenChatSelectOption<ToolPermissionRule>(
          value: ToolPermissionRule.allow,
          label: l10n.projectToolRuleAllow,
        ),
        OpenChatSelectOption<ToolPermissionRule>(
          value: ToolPermissionRule.deny,
          label: l10n.projectToolRuleDeny,
        ),
      ],
    );
  }
}
