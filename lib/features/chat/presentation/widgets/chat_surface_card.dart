import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

class ChatSurfaceCard extends StatelessWidget {
  const ChatSurfaceCard({
    required this.child,
    this.padding = EdgeInsets.zero,
    super.key,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final radius = BorderRadius.circular(OpenChatRadii.card);

    return Container(
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        color: palette.selected,
        borderRadius: radius,
        border: Border.all(color: palette.border),
      ),
      child: Padding(padding: padding, child: child),
    );
  }
}
