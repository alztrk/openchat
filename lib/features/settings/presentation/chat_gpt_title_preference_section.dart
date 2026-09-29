import 'package:flutter/material.dart';

import '../../../../app/openchat_select.dart';
import '../../../../app/openchat_theme.dart';
import '../../../../l10n/openchat_localizations.dart';
import '../../chat/domain/chatgpt_connection.dart';

class ChatGptTitlePreferenceSection extends StatelessWidget {
  const ChatGptTitlePreferenceSection({
    required this.preference,
    required this.connections,
    required this.palette,
    required this.isLoading,
    required this.hasError,
    required this.isSaving,
    required this.onRetry,
    required this.onConnectionChanged,
    required this.onWorkspaceChanged,
    super.key,
  });

  final ChatGptTitlePreference preference;
  final List<ChatGptConnection> connections;
  final OpenChatPalette palette;
  final bool isLoading;
  final bool hasError;
  final bool isSaving;
  final VoidCallback onRetry;
  final ValueChanged<String?> onConnectionChanged;
  final ValueChanged<String?> onWorkspaceChanged;

  @override
  Widget build(BuildContext context) {
    final preference = this.preference;
    final connections = this.connections;
    final isLoading = this.isLoading;
    final hasError = this.hasError;
    final isSaving = this.isSaving;
    final onRetry = this.onRetry;
    final onConnectionChanged = this.onConnectionChanged;
    final onWorkspaceChanged = this.onWorkspaceChanged;
    final l10n = context.openchatL10n;
    final connectionId = preference.connectionId;
    ChatGptConnection? titleConnection;
    for (final connection in connections) {
      if (connection.id == connectionId) {
        titleConnection = connection;
        break;
      }
    }
    final connectionItems = <OpenChatSelectOption<String>>[
      OpenChatSelectOption<String>(
        value: '',
        label: l10n.titleUseConversationAccount,
      ),
      for (final connection in connections)
        OpenChatSelectOption<String>(
          value: connection.id,
          label: connection.email ?? l10n.accountEmailUnavailable,
        ),
      if (connectionId != null && titleConnection == null)
        OpenChatSelectOption<String>(
          value: connectionId,
          enabled: false,
          label: l10n.titleAccountUnavailable,
        ),
    ];
    final selectedConnectionValue =
        connectionId != null &&
            connectionItems.any((item) => item.value == connectionId)
        ? connectionId
        : '';
    final workspaces =
        titleConnection?.workspaces ?? const <ChatGptWorkspace>[];
    final selectedWorkspaceId = preference.workspaceId;
    final selectedWorkspaceIsAvailable = workspaces.any(
      (workspace) => workspace.id == selectedWorkspaceId,
    );

    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 18, 12, 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            l10n.titleGenerationTarget,
            style: TextStyle(
              color: palette.text,
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            l10n.titleGenerationTargetDescription,
            style: TextStyle(color: palette.secondaryText, fontSize: 12),
          ),
          if (isLoading)
            const Padding(
              padding: EdgeInsets.only(top: 10),
              child: LinearProgressIndicator(),
            )
          else if (hasError)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      l10n.titlePreferenceLoadFailed,
                      style: TextStyle(
                        color: palette.secondaryText,
                        fontSize: 12,
                      ),
                    ),
                  ),
                  TextButton.icon(
                    onPressed: onRetry,
                    icon: const Icon(Icons.refresh_rounded, size: 16),
                    label: Text(l10n.retry),
                  ),
                ],
              ),
            )
          else ...[
            const SizedBox(height: 8),
            OpenChatSelect<String>(
              options: connectionItems,
              value: selectedConnectionValue,
              onChanged: isSaving ? null : onConnectionChanged,
              palette: palette,
              width: double.infinity,
              menuWidth: 360,
              height: 40,
            ),
            if (isSaving)
              const Padding(
                padding: EdgeInsets.only(top: 8),
                child: LinearProgressIndicator(),
              ),
            if (connectionId != null && titleConnection != null) ...[
              if (workspaces.length > 1 ||
                  (workspaces.length == 1 && !selectedWorkspaceIsAvailable))
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: OpenChatSelect<String>(
                    options: [
                      for (final workspace in workspaces)
                        OpenChatSelectOption<String>(
                          value: workspace.id,
                          label:
                              workspace.displayName ??
                              l10n.workspaceWithoutName,
                        ),
                    ],
                    value: selectedWorkspaceIsAvailable
                        ? selectedWorkspaceId
                        : null,
                    onChanged: isSaving ? null : onWorkspaceChanged,
                    palette: palette,
                    width: double.infinity,
                    menuWidth: 360,
                    height: 40,
                    hint: l10n.titleWorkspaceHint,
                  ),
                )
              else if (workspaces.isEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    l10n.workspaceUnavailable,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                )
              else if (workspaces.length == 1)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    workspaces.single.displayName ?? l10n.workspace,
                    style: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 12,
                    ),
                  ),
                ),
            ],
          ],
        ],
      ),
    );
  }
}
