import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

import 'package:openchat/features/chat/domain/chatgpt_connection.dart';

class ChatGptOAuthConnectionRow extends StatelessWidget {
  const ChatGptOAuthConnectionRow({
    required this.connection,
    required this.isRemoving,
    required this.isSigningIn,
    required this.selectedWorkspaceId,
    required this.onUseConnection,
    required this.onRemove,
    required this.onWorkspaceChanged,
    super.key,
  });

  final ChatGptConnection connection;
  final bool isRemoving;
  final bool isSigningIn;
  final String? selectedWorkspaceId;
  final VoidCallback onUseConnection;
  final VoidCallback onRemove;
  final ValueChanged<String>? onWorkspaceChanged;

  @override
  Widget build(BuildContext context) {
    final connection = this.connection;
    final isRemoving = this.isRemoving;
    final isSigningIn = this.isSigningIn;
    final selectedWorkspaceId = this.selectedWorkspaceId;
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final accountName = connection.email ?? l10n.accountEmailUnavailable;
    final planType = connection.planType;
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 5),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      decoration: BoxDecoration(
        color: connection.isSelected ? palette.selected : palette.raisedSurface,
        borderRadius: BorderRadius.circular(OpenChatRadii.card),
        border: Border.all(
          color: connection.isSelected ? palette.accent : palette.subtleBorder,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                LucideIcons.circleUserRound,
                size: 20,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      accountName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      connection.authStatus == 'active'
                          ? l10n.accountPlan(planType ?? l10n.planUnavailable)
                          : l10n.connectionNeedsSignIn,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              connection.isSelected
                  ? TextButton.icon(
                      onPressed: null,
                      icon: const Icon(LucideIcons.circleCheck, size: 16),
                      label: Text(l10n.connectionSelected),
                    )
                  : OutlinedButton(
                      onPressed: isSigningIn || isRemoving
                          ? null
                          : onUseConnection,
                      child: Text(l10n.useConnection),
                    ),
              IconButton(
                tooltip: l10n.removeChatGptConnection,
                onPressed: isRemoving || isSigningIn ? null : onRemove,
                icon: isRemoving
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(LucideIcons.trash2, size: 18),
              ),
            ],
          ),
          if (connection.isSelected && connection.workspaces.length > 1)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 10),
              child: OpenChatSelect<String>(
                options: [
                  for (final workspace in connection.workspaces)
                    OpenChatSelectOption<String>(
                      value: workspace.id,
                      label: workspace.displayName ?? l10n.workspaceWithoutName,
                    ),
                ],
                value:
                    connection.workspaces.any(
                      (item) => item.id == selectedWorkspaceId,
                    )
                    ? selectedWorkspaceId
                    : null,
                onChanged: isSigningIn || isRemoving
                    ? null
                    : onWorkspaceChanged,
                palette: palette,
                menuWidth: 320,
                height: 40,
                hint: l10n.selectWorkspace,
              ),
            )
          else if (connection.isSelected && connection.workspaces.isEmpty)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                l10n.workspaceUnavailable,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            ),
        ],
      ),
    );
  }
}
