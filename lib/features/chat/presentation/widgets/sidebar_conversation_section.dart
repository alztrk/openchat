import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_section_heading.dart';

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

  @override
  Widget build(BuildContext context) {
    return DragTarget<String>(
      onWillAcceptWithDetails: (_) => onDropConversation != null,
      onAcceptWithDetails: (details) => onDropConversation?.call(details.data),
      builder: (context, candidates, _) {
        final isDropTarget = candidates.isNotEmpty;
        final palette = OpenChatPalette.of(context);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              padding: const EdgeInsets.symmetric(horizontal: 6),
              decoration: BoxDecoration(
                color: isDropTarget ? palette.hover : Colors.transparent,
                borderRadius: BorderRadius.circular(6),
              ),
              child: Row(
                children: [
                  Expanded(child: SidebarSectionHeading(title: title)),
                  if (isDropTarget)
                    Icon(dropIcon, size: 16, color: palette.accentIcon),
                ],
              ),
            ),
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
              for (var index = 0; index < conversations.length; index++) ...[
                if (index > 0) const SizedBox(height: 2),
                DraggableSidebarConversation(
                  conversation: conversations[index],
                  child: SidebarConversationTile(
                    conversation: conversations[index],
                    selected: conversations[index].id == selectedConversationId,
                    height: itemHeight,
                    inset: 8,
                    showChatIcon: false,
                    onPressed: onSelectConversation == null
                        ? null
                        : () => onSelectConversation!(conversations[index].id),
                    onTogglePinned: onTogglePinned == null
                        ? null
                        : () => onTogglePinned!(conversations[index].id),
                    onRename: onRenameConversation == null
                        ? null
                        : () => onRenameConversation!(conversations[index].id),
                    onDelete: onDeleteConversation == null
                        ? null
                        : () => onDeleteConversation!(conversations[index].id),
                    onExport: onExportConversation == null
                        ? null
                        : () => onExportConversation!(conversations[index].id),
                    key: ValueKey<String>(
                      'sidebar-conversation-${conversations[index].id}',
                    ),
                  ),
                ),
              ],
          ],
        );
      },
    );
  }
}
