import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:openchat/app/openchat_theme.dart';

class OpenChatDropdown extends StatelessWidget {
  const OpenChatDropdown({
    required this.palette,
    required this.menuChildren,
    required this.builder,
    this.controller,
    this.menuPadding = const EdgeInsets.all(4),
    this.alignmentOffset = const Offset(0, 6),
    this.reservedPadding = const EdgeInsets.all(12),
    this.maximumSize,
    this.crossAxisUnconstrained = true,
    this.onOpen,
    this.onClose,
    super.key,
  });

  final OpenChatPalette palette;
  final List<Widget> menuChildren;
  final Widget Function(
    BuildContext context,
    MenuController controller,
    Widget? child,
  )
  builder;
  final MenuController? controller;
  final EdgeInsetsGeometry menuPadding;
  final Offset alignmentOffset;
  final EdgeInsets reservedPadding;
  final Size? maximumSize;
  final bool crossAxisUnconstrained;
  final VoidCallback? onOpen;
  final VoidCallback? onClose;

  static MenuStyle menuStyle(
    OpenChatPalette palette, {
    EdgeInsetsGeometry padding = const EdgeInsets.all(4),
    Size? maximumSize,
  }) {
    return MenuStyle(
      backgroundColor: WidgetStatePropertyAll(palette.surface),
      elevation: const WidgetStatePropertyAll(6),
      padding: WidgetStatePropertyAll(padding),
      side: WidgetStatePropertyAll(BorderSide(color: palette.border)),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.menu),
        ),
      ),
      maximumSize: WidgetStatePropertyAll(maximumSize),
    );
  }

  static ButtonStyle menuItemStyle(OpenChatPalette palette) {
    return ButtonStyle(
      minimumSize: const WidgetStatePropertyAll(Size(0, 42)),
      padding: const WidgetStatePropertyAll(EdgeInsets.zero),
      overlayColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.disabled)) return Colors.transparent;
        if (states.contains(WidgetState.pressed)) return palette.selected;
        if (states.contains(WidgetState.hovered) ||
            states.contains(WidgetState.focused)) {
          return palette.hover;
        }
        return Colors.transparent;
      }),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return MenuAnchor(
      controller: controller,
      style: menuStyle(palette, padding: menuPadding, maximumSize: maximumSize),
      alignmentOffset: alignmentOffset,
      reservedPadding: reservedPadding,
      crossAxisUnconstrained: crossAxisUnconstrained,
      onOpen: onOpen,
      onClose: onClose,
      menuChildren: [
        for (var index = 0; index < menuChildren.length; index++)
          Focus(
            autofocus: index == 0,
            skipTraversal: true,
            child: menuChildren[index],
          ),
      ],
      builder: (context, controller, child) => Focus(
        skipTraversal: true,
        onKeyEvent: (_, event) {
          if (controller.isOpen &&
              event is KeyDownEvent &&
              event.logicalKey == LogicalKeyboardKey.escape) {
            controller.close();
            return KeyEventResult.handled;
          }
          return KeyEventResult.ignored;
        },
        child: builder(context, controller, child),
      ),
    );
  }
}
