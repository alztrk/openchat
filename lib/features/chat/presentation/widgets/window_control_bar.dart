import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/window_controls.dart';

class WindowControlBar extends StatefulWidget {
  const WindowControlBar({super.key});

  @override
  State<WindowControlBar> createState() => _WindowControlBarState();
}

class _WindowControlBarState extends State<WindowControlBar> {
  bool _maximized = false;

  Future<void> _toggleMaximize() async {
    await OpenChatWindowControls.toggleMaximize();
    if (mounted) {
      setState(() => _maximized = !_maximized);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final iconRoot = dark
        ? 'assets/icons/window/dark'
        : 'assets/icons/window/light';

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _WindowButton(
          label: l10n.minimizeWindow,
          iconPath: '$iconRoot/minimize.svg',
          iconSize: const Size(10.667, 1.333),
          color: palette.secondaryText,
          onPressed: () => unawaited(OpenChatWindowControls.minimize()),
        ),
        const SizedBox(width: 8),
        _WindowButton(
          label: _maximized ? l10n.restoreWindow : l10n.maximizeWindow,
          iconPath: '$iconRoot/maximize.svg',
          iconSize: const Size(10.667, 10.667),
          color: palette.secondaryText,
          onPressed: _toggleMaximize,
        ),
        const SizedBox(width: 8),
        _WindowButton(
          label: l10n.close,
          iconPath: '$iconRoot/close.svg',
          iconSize: const Size(10.667, 10.667),
          color: palette.secondaryText,
          onPressed: () => unawaited(OpenChatWindowControls.close()),
        ),
      ],
    );
  }
}

class _WindowButton extends StatelessWidget {
  const _WindowButton({
    required this.label,
    required this.iconPath,
    required this.iconSize,
    required this.color,
    required this.onPressed,
  });

  final String label;
  final String iconPath;
  final Size iconSize;
  final Color color;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      onPressed: onPressed,
      tooltip: label,
      constraints: const BoxConstraints.tightFor(width: 32, height: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      icon: SvgPicture.asset(
        iconPath,
        width: iconSize.width,
        height: iconSize.height,
        colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
        excludeFromSemantics: true,
      ),
    );
  }
}
