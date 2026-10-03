import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_dropdown.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';

class DraggableSidebarConversation extends StatelessWidget {
  const DraggableSidebarConversation({
    super.key,
    required this.conversation,
    required this.child,
  });

  final ConversationSidebarConversation conversation;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return Draggable<String>(
      data: conversation.id,
      maxSimultaneousDrags: 1,
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 280),
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
          decoration: BoxDecoration(
            color: palette.composer,
            border: Border.all(color: palette.border),
            borderRadius: BorderRadius.circular(7),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.chat_bubble_outline_rounded,
                size: 15,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 8),
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

  @override
  State<SidebarConversationTile> createState() =>
      _SidebarConversationTileState();
}

class _SidebarConversationTileState extends State<SidebarConversationTile> {
  bool _hovered = false;
  bool _menuOpen = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final l10n = context.openchatL10n;
    final onRename = widget.onRename;
    final onExport = widget.onExport;
    final onDelete = widget.onDelete;
    final hasActions =
        widget.onTogglePinned != null ||
        widget.onRename != null ||
        widget.onDelete != null ||
        widget.onExport != null;
    final showActions =
        hasActions &&
        (_hovered ||
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
        label: widget.conversation.title,
        child: Material(
          color: widget.selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(7),
          child: InkWell(
            onTap: widget.onPressed,
            hoverColor: palette.selected,
            borderRadius: BorderRadius.circular(7),
            child: SizedBox(
              height: hasActions ? 40 : widget.height,
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: widget.inset),
                child: Row(
                  children: [
                    if (widget.showChatIcon) ...[
                      Icon(
                        Icons.chat_bubble_outline_rounded,
                        size: 15,
                        color: palette.secondaryIcon,
                      ),
                      const SizedBox(width: 8),
                    ],
                    Expanded(
                      child: Text(
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
                    ),
                    if (hasActions) ...[
                      const SizedBox(width: 4),
                      IgnorePointer(
                        ignoring: !showActions,
                        child: AnimatedOpacity(
                          opacity: showActions ? 1 : 0,
                          duration: const Duration(milliseconds: 120),
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
                                    width: 32,
                                    height: 36,
                                  ),
                                  onPressed: widget.onTogglePinned,
                                  icon: Icon(
                                    widget.conversation.isPinned
                                        ? Icons.push_pin_rounded
                                        : Icons.push_pin_outlined,
                                    size: 15,
                                  ),
                                ),
                              if (widget.onRename != null ||
                                  widget.onDelete != null ||
                                  widget.onExport != null)
                                SizedBox(
                                  width: 32,
                                  height: 36,
                                  child: OpenChatDropdown(
                                    palette: palette,
                                    alignmentOffset: const Offset(-152, 6),
                                    onOpen: () =>
                                        setState(() => _menuOpen = true),
                                    onClose: () =>
                                        setState(() => _menuOpen = false),
                                    menuChildren: [
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
                                    builder: (context, controller, _) =>
                                        IconButton(
                                          tooltip: l10n.moreOptions,
                                          visualDensity: VisualDensity.compact,
                                          padding: EdgeInsets.zero,
                                          constraints:
                                              const BoxConstraints.tightFor(
                                                width: 32,
                                                height: 36,
                                              ),
                                          onPressed: () => controller.isOpen
                                              ? controller.close()
                                              : controller.open(),
                                          icon: const Icon(
                                            Icons.more_horiz_rounded,
                                            size: 17,
                                          ),
                                        ),
                                  ),
                                ),
                            ],
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
