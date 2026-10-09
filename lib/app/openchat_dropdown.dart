import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:openchat/app/openchat_theme.dart';

class OpenChatDropdown extends StatefulWidget {
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
    FocusNode focusNode,
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
      side: WidgetStatePropertyAll(BorderSide(color: palette.controlBorder)),
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
        if (states.contains(WidgetState.focused)) return palette.hover;
        if (states.contains(WidgetState.hovered)) return palette.hover;
        return Colors.transparent;
      }),
      side: const WidgetStatePropertyAll(BorderSide.none),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
        ),
      ),
    );
  }

  @override
  State<OpenChatDropdown> createState() => _OpenChatDropdownState();
}

class _OpenChatDropdownState extends State<OpenChatDropdown> {
  MenuController? _activeMenuController;
  late final FocusNode _triggerFocusNode = FocusNode(
    debugLabel: 'open chat dropdown trigger',
  );
  late final FocusScopeNode _menuFocusScope = FocusScopeNode(
    debugLabel: 'open chat dropdown menu',
  );
  bool _listeningForEscape = false;

  bool _handleKeyEvent(KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape &&
        (_activeMenuController?.isOpen ?? false)) {
      _activeMenuController?.close();
      _triggerFocusNode.requestFocus();
      return true;
    }
    return false;
  }

  void _handleMenuOpen() {
    if (!_listeningForEscape) {
      HardwareKeyboard.instance.addHandler(_handleKeyEvent);
      _listeningForEscape = true;
    }
    widget.onOpen?.call();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !(_activeMenuController?.isOpen ?? false)) return;
      final context = _menuFocusScope.context;
      if (context == null) return;
      final policy =
          FocusTraversalGroup.maybeOf(context) ?? ReadingOrderTraversalPolicy();
      policy
          .findFirstFocus(_menuFocusScope, ignoreCurrentFocus: true)
          ?.requestFocus();
    });
  }

  void _handleMenuClose() {
    if (_listeningForEscape) {
      HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
      _listeningForEscape = false;
    }
    widget.onClose?.call();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || ModalRoute.isCurrentOf(context) != true) return;
      _triggerFocusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    if (_listeningForEscape) {
      HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    }
    _triggerFocusNode.dispose();
    _menuFocusScope.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MenuAnchor(
      controller: widget.controller,
      childFocusNode: _triggerFocusNode,
      style: OpenChatDropdown.menuStyle(
        widget.palette,
        padding: widget.menuPadding,
        maximumSize: widget.maximumSize,
      ),
      alignmentOffset: widget.alignmentOffset,
      reservedPadding: widget.reservedPadding,
      crossAxisUnconstrained: widget.crossAxisUnconstrained,
      onOpen: _handleMenuOpen,
      onClose: _handleMenuClose,
      menuChildren: [
        FocusScope(
          node: _menuFocusScope,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: widget.menuChildren,
          ),
        ),
      ],
      builder: (context, controller, child) {
        _activeMenuController = controller;
        return widget.builder(context, controller, child, _triggerFocusNode);
      },
    );
  }
}
