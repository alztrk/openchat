import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_collapsible_heading.dart';

class SidebarConversationSection extends StatelessWidget {
  const SidebarConversationSection({
    super.key,
    required this.title,
    required this.emptyMessage,
    required this.conversations,
    required this.selectedConversationId,
    required this.itemHeight,
    required this.dropIcon,
    required this.onSelectConversation,
    required this.onTogglePinned,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onExportConversation,
    required this.onDropConversation,
    this.isArchivedSection = false,
    this.headingIcon,
    this.onArchiveConversation,
    this.onEditConversationTags,
    this.onToggleConversationBookmark,
    this.collapsed = false,
    this.onToggleCollapsed,
    this.onCreateConversation,
    this.selectionMode = false,
    this.selectedConversationIds = const <String>{},
    this.onToggleBatchSelection,
  });

  final String title;
  final String emptyMessage;
  final List<ConversationSidebarConversation> conversations;
  final String? selectedConversationId;
  final double itemHeight;
  final IconData dropIcon;
  final ValueChanged<String>? onSelectConversation;
  final ValueChanged<String>? onTogglePinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final ValueChanged<String>? onDropConversation;
  final bool isArchivedSection;
  final IconData? headingIcon;
  final ValueChanged<String>? onArchiveConversation;
  final ValueChanged<String>? onEditConversationTags;
  final ValueChanged<String>? onToggleConversationBookmark;
  final bool collapsed;
  final VoidCallback? onToggleCollapsed;
  final VoidCallback? onCreateConversation;
  final bool selectionMode;
  final Set<String> selectedConversationIds;
  final ValueChanged<String>? onToggleBatchSelection;

  @override
  Widget build(BuildContext context) {
    return DragTarget<String>(
      onWillAcceptWithDetails: (_) => onDropConversation != null,
      onAcceptWithDetails: (details) => onDropConversation?.call(details.data),
      builder: (context, candidates, _) {
        final isDropTarget = candidates.isNotEmpty;
        final palette = OpenChatPalette.of(context);
        final l10n = context.openchatL10n;
        final touch = switch (Theme.of(context).platform) {
          TargetPlatform.android ||
          TargetPlatform.iOS ||
          TargetPlatform.fuchsia => true,
          _ => false,
        };
        final trailing = <Widget>[
          if (isDropTarget) Icon(dropIcon, size: 16, color: palette.accentIcon),
          if (onCreateConversation case final createConversation?)
            IconButton(
              key: const ValueKey<String>('conversations-create-button'),
              tooltip: l10n.newChat,
              onPressed: createConversation,
              visualDensity: VisualDensity.standard,
              constraints: BoxConstraints.tightFor(
                width: touch ? 44 : 32,
                height: touch ? 44 : 32,
              ),
              padding: EdgeInsets.zero,
              icon: const Icon(LucideIcons.plus, size: 19),
            ),
        ];
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            AnimatedContainer(
              duration: MediaQuery.disableAnimationsOf(context)
                  ? Duration.zero
                  : const Duration(milliseconds: 120),
              decoration: BoxDecoration(
                color: isDropTarget ? palette.hover : Colors.transparent,
                borderRadius: BorderRadius.circular(OpenChatRadii.button),
              ),
              child: SidebarCollapsibleHeading(
                title: title,
                icon: headingIcon,
                collapsed: collapsed,
                onPressed: onToggleCollapsed ?? () {},
                trailing: trailing.isEmpty
                    ? null
                    : Row(mainAxisSize: MainAxisSize.min, children: trailing),
              ),
            ),
            AnimatedSize(
              duration: MediaQuery.disableAnimationsOf(context)
                  ? Duration.zero
                  : const Duration(milliseconds: 180),
              curve: Curves.easeOutCubic,
              alignment: Alignment.topCenter,
              child: ClipRect(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (!collapsed) ...[
                      const SizedBox(height: 4),
                      if (conversations.isEmpty)
                        Padding(
                          padding: const EdgeInsets.fromLTRB(4, 0, 4, 4),
                          child: Text(
                            emptyMessage,
                            style: TextStyle(
                              color: palette.secondaryText,
                              fontSize: 12,
                              height: 18 / 12,
                            ),
                          ),
                        )
                      else
                        for (
                          var index = 0;
                          index < conversations.length;
                          index++
                        ) ...[
                          if (index > 0) const SizedBox(height: 2),
                          DraggableSidebarConversation(
                            conversation: conversations[index],
                            enabled: !isArchivedSection && !selectionMode,
                            child: SidebarConversationTile(
                              conversation: conversations[index],
                              selected:
                                  conversations[index].id ==
                                  selectedConversationId,
                              selectionMode:
                                  selectionMode && !isArchivedSection,
                              selectedForBatch: selectedConversationIds
                                  .contains(conversations[index].id),
                              height: itemHeight,
                              inset: 8,
                              showChatIcon: false,
                              onPressed: selectionMode && !isArchivedSection
                                  ? () => onToggleBatchSelection?.call(
                                      conversations[index].id,
                                    )
                                  : onSelectConversation == null
                                  ? null
                                  : () => onSelectConversation!(
                                      conversations[index].id,
                                    ),
                              onTogglePinned:
                                  isArchivedSection || onTogglePinned == null
                                  ? null
                                  : () => onTogglePinned!(
                                      conversations[index].id,
                                    ),
                              onRename: onRenameConversation == null
                                  ? null
                                  : () => onRenameConversation!(
                                      conversations[index].id,
                                    ),
                              onDelete: onDeleteConversation == null
                                  ? null
                                  : () => onDeleteConversation!(
                                      conversations[index].id,
                                    ),
                              onExport: onExportConversation == null
                                  ? null
                                  : () => onExportConversation!(
                                      conversations[index].id,
                                    ),
                              onArchive: onArchiveConversation == null
                                  ? null
                                  : () => onArchiveConversation!(
                                      conversations[index].id,
                                    ),
                              onEditTags: onEditConversationTags == null
                                  ? null
                                  : () => onEditConversationTags!(
                                      conversations[index].id,
                                    ),
                              onToggleBookmark:
                                  onToggleConversationBookmark == null
                                  ? null
                                  : () => onToggleConversationBookmark!(
                                      conversations[index].id,
                                    ),
                              key: ValueKey<String>(
                                'sidebar-conversation-${conversations[index].id}',
                              ),
                            ),
                          ),
                        ],
                    ],
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}
