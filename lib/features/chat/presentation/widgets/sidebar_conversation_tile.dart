import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';

class DraggableSidebarConversation extends StatelessWidget {
  const DraggableSidebarConversation({
    super.key,
    required this.conversation,
    required this.child,
    this.enabled = true,
  });

  final ConversationSidebarConversation conversation;
  final Widget child;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    if (!enabled) return child;
    final palette = OpenChatPalette.of(context);

    return Draggable<String>(
      data: conversation.id,
      maxSimultaneousDrags: 1,
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 280),
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          decoration: BoxDecoration(
            color: palette.composer,
            border: Border.all(color: palette.border),
            borderRadius: BorderRadius.circular(6),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                LucideIcons.messageCircle,
                size: 14,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 6),
              Flexible(
                child: Text(
                  conversation.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.text, fontSize: 13),
                ),
              ),
            ],
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.45, child: child),
      child: child,
    );
  }
}

class SidebarConversationTile extends StatefulWidget {
  const SidebarConversationTile({
    required this.conversation,
    required this.selected,
    required this.height,
    required this.inset,
    required this.onPressed,
    this.showChatIcon = false,
    this.onTogglePinned,
    this.onRename,
    this.onDelete,
    this.onExport,
    this.onArchive,
    this.onEditTags,
    this.onToggleBookmark,
    this.selectionMode = false,
    this.selectedForBatch = false,
    super.key,
  });

  final ConversationSidebarConversation conversation;
  final bool selected;
  final double height;
  final double inset;
  final VoidCallback? onPressed;
  final bool showChatIcon;
  final VoidCallback? onTogglePinned;
  final VoidCallback? onRename;
  final VoidCallback? onDelete;
  final VoidCallback? onExport;
  final VoidCallback? onArchive;
  final VoidCallback? onEditTags;
  final VoidCallback? onToggleBookmark;
  final bool selectionMode;
  final bool selectedForBatch;

  @override
  State<SidebarConversationTile> createState() =>
      _SidebarConversationTileState();
}

class _SidebarConversationTileState extends State<SidebarConversationTile> {
  bool _hovered = false;
  bool _focused = false;
  bool _menuOpen = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final onRename = widget.onRename;
    final onExport = widget.onExport;
    final onDelete = widget.onDelete;
    final hasActions =
        !widget.selectionMode &&
        (widget.onTogglePinned != null ||
            widget.onRename != null ||
            widget.onDelete != null ||
            widget.onExport != null ||
            widget.onArchive != null ||
            widget.onEditTags != null ||
            widget.onToggleBookmark != null);
    final hasMenuAction =
        !widget.selectionMode &&
        (widget.onRename != null ||
            widget.onDelete != null ||
            widget.onExport != null ||
            widget.onArchive != null ||
            widget.onEditTags != null ||
            widget.onToggleBookmark != null);
    final showActions =
        hasActions &&
        (_hovered ||
            _focused ||
            widget.onPressed == null ||
            widget.selected ||
            _menuOpen ||
            MediaQuery.sizeOf(context).width <
                OpenChatSpacing.sidebarBreakpoint);

    return MouseRegion(
      onEnter: hasActions ? (_) => setState(() => _hovered = true) : null,
      onExit: hasActions ? (_) => setState(() => _hovered = false) : null,
      child: Semantics(
        button: true,
        enabled: widget.onPressed != null,
        selected: widget.selected,
        label: <String>[
          widget.conversation.title,
          ...widget.conversation.tags,
          if (widget.conversation.isBookmarked) l10n.conversationBookmarked,
        ].join(', '),
        onTap: widget.onPressed,
        child: Material(
          color: widget.selected ? palette.selected : Colors.transparent,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(6),
            side: _focused
                ? BorderSide(color: palette.focusRing, width: 2)
                : BorderSide.none,
          ),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: widget.onPressed,
            onFocusChange: (focused) => setState(() => _focused = focused),
            hoverColor: palette.selected,
            borderRadius: BorderRadius.circular(6),
            child: ConstrainedBox(
              constraints: BoxConstraints(minHeight: 44),
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: widget.inset),
                child: Row(
                  children: [
                    if (widget.selectionMode) ...[
                      Semantics(
                        label: l10n.selectConversation(
                          widget.conversation.title,
                        ),
                        child: Checkbox(
                          value: widget.selectedForBatch,
                          onChanged: widget.onPressed == null
                              ? null
                              : (_) => widget.onPressed!(),
                          visualDensity: VisualDensity.compact,
                          materialTapTargetSize:
                              MaterialTapTargetSize.shrinkWrap,
                        ),
                      ),
                      const SizedBox(width: 4),
                    ],
                    if (widget.showChatIcon) ...[
                      Icon(
                        LucideIcons.messageCircle,
                        size: 14,
                        color: palette.secondaryIcon,
                      ),
                      const SizedBox(width: 6),
                    ],
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Text(
                            widget.conversation.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: palette.text,
                              fontSize: 13,
                              fontWeight: widget.selected
                                  ? FontWeight.w500
                                  : FontWeight.w400,
                              height: 18 / 13,
                            ),
                          ),
                        ],
                      ),
                    ),
                    if (widget.conversation.tags.isNotEmpty)
                      Tooltip(
                        message: widget.conversation.tags.join(', '),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 4),
                          child: Icon(
                            LucideIcons.tags,
                            size: 13,
                            color: palette.secondaryIcon,
                          ),
                        ),
                      ),
                    if (widget.conversation.isBookmarked)
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 4),
                        child: Icon(
                          LucideIcons.bookmark,
                          size: 13,
                          color: palette.accentIcon,
                        ),
                      ),
                    if (hasActions) ...[
                      const SizedBox(width: 2),
                      ExcludeFocus(
                        excluding: !showActions,
                        child: IgnorePointer(
                          ignoring: !showActions,
                          child: AnimatedOpacity(
                            opacity: showActions ? 1 : 0,
                            duration: MediaQuery.disableAnimationsOf(context)
                                ? Duration.zero
                                : const Duration(milliseconds: 120),
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                if (widget.onTogglePinned != null)
                                  IconButton(
                                    tooltip: widget.conversation.isPinned
                                        ? l10n.unpinConversation
                                        : l10n.pinConversation,
                                    visualDensity: VisualDensity.compact,
                                    padding: EdgeInsets.zero,
                                    constraints: const BoxConstraints.tightFor(
                                      width: 44,
                                      height: 44,
                                    ),
                                    onPressed: widget.onTogglePinned,
                                    icon: Icon(
                                      widget.conversation.isPinned
                                          ? LucideIcons.pin
                                          : LucideIcons.pinOff,
                                      size: 15,
                                    ),
                                  ),
                                if (hasMenuAction)
                                  SizedBox(
                                    width: 44,
                                    height: 44,
                                    child: OpenChatDropdown(
                                      palette: palette,
                                      alignmentOffset: const Offset(-152, 6),
                                      onOpen: () =>
                                          setState(() => _menuOpen = true),
                                      onClose: () =>
                                          setState(() => _menuOpen = false),
                                      menuChildren: [
                                        if (widget.onToggleBookmark
                                            case final onToggleBookmark?)
                                          _menuActionItem(
                                            palette: palette,
                                            label:
                                                widget.conversation.isBookmarked
                                                ? l10n.removeConversationBookmark
                                                : l10n.bookmarkConversation,
                                            onPressed: onToggleBookmark,
                                          ),
                                        if (widget.onEditTags
                                            case final onEditTags?)
                                          _menuActionItem(
                                            palette: palette,
                                            label: l10n.editConversationTags,
                                            onPressed: onEditTags,
                                          ),
                                        if (onRename != null)
                                          _menuActionItem(
                                            palette: palette,
                                            label: l10n.renameConversation,
                                            onPressed: onRename,
                                          ),
                                        if (onExport != null)
                                          _menuActionItem(
                                            palette: palette,
                                            label: l10n.exportConversation,
                                            onPressed: onExport,
                                          ),
                                        if (widget.onArchive
                                            case final onArchive?)
                                          _menuActionItem(
                                            palette: palette,
                                            label:
                                                widget.conversation.isArchived
                                                ? l10n.restoreConversation
                                                : l10n.archiveConversation,
                                            onPressed: onArchive,
                                          ),
                                        if (onDelete != null)
                                          _menuActionItem(
                                            palette: palette,
                                            label: l10n.deleteConversation,
                                            foregroundColor: Theme.of(context)
                                                .colorScheme
                                                .error,
                                            onPressed: onDelete,
                                          ),
                                      ],
                                      builder:
                                          (
                                            context,
                                            controller,
                                            _,
                                            focusNode,
                                          ) => IconButton(
                                            tooltip: l10n.moreOptions,
                                            focusNode: focusNode,
                                            visualDensity:
                                                VisualDensity.compact,
                                            padding: EdgeInsets.zero,
                                            constraints:
                                                const BoxConstraints.tightFor(
                                                  width: 44,
                                                  height: 44,
                                                ),
                                            onPressed: () => controller.isOpen
                                                ? controller.close()
                                                : controller.open(),
                                            icon: const Icon(
                                              LucideIcons.ellipsis,
                                              size: 17,
                                            ),
                                          ),
                                    ),
                                  ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _menuActionItem({
    required OpenChatPalette palette,
    required String label,
    required VoidCallback onPressed,
    Color? foregroundColor,
  }) {
    return MenuItemButton(
      onPressed: onPressed,
      style: OpenChatDropdown.menuItemStyle(palette),
      child: SizedBox(
        width: 184,
        height: 42,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(label, style: TextStyle(color: foregroundColor)),
          ),
        ),
      ),
    );
  }
}
