import 'package:flutter/material.dart';

import '../../../../app/openchat_theme.dart';
import '../../../../l10n/openchat_localizations.dart';
import '../../chat/domain/chatgpt_connection.dart';

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
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                Icons.account_circle_outlined,
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
                      icon: const Icon(Icons.check_circle_outline, size: 16),
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
                    : const Icon(Icons.delete_outline_rounded, size: 18),
              ),
            ],
          ),
          if (connection.isSelected && connection.workspaces.length > 1)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 10),
              child: DropdownButton<String>(
                value:
                    connection.workspaces.any(
                      (item) => item.id == selectedWorkspaceId,
                    )
                    ? selectedWorkspaceId
                    : null,
                isExpanded: true,
                hint: Text(l10n.selectWorkspace),
                items: connection.workspaces
                    .map(
                      (workspace) => DropdownMenuItem<String>(
                        value: workspace.id,
                        child: Text(
                          workspace.displayName ?? l10n.workspaceWithoutName,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    )
                    .toList(growable: false),
                onChanged: isSigningIn || isRemoving
                    ? null
                    : (workspaceId) {
                        if (workspaceId != null) {
                          onWorkspaceChanged?.call(workspaceId);
                        }
                      },
              ),
            )
          else if (connection.isSelected && connection.workspaces.isEmpty)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                l10n.workspaceUnavailable,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            )
          else if (!connection.isSelected && connection.workspaces.length > 1)
            Padding(
              padding: const EdgeInsets.only(left: 30, top: 8),
              child: Text(
                l10n.selectAccountForWorkspace,
                style: TextStyle(color: palette.secondaryText, fontSize: 12),
              ),
            ),
        ],
      ),
    );
  }
}
