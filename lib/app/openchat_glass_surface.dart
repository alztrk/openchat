import 'dart:ui' show ImageFilter;

import 'package:flutter/material.dart';

import 'openchat_theme.dart';

/// Adds a restrained translucent surface treatment to elevated UI.
///
/// The opaque high-contrast path keeps borders and text legible while the
/// regular path lets the surface inherit a small amount of depth from the
/// content behind it.
class OpenChatGlassSurface extends StatelessWidget {
  const OpenChatGlassSurface({
    required this.child,
    this.borderRadius = const BorderRadius.all(
      Radius.circular(OpenChatRadii.panel),
    ),
    this.padding,
    super.key,
  });

  final Widget child;
  final BorderRadius borderRadius;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final highContrast = MediaQuery.highContrastOf(context);
    final surfaceColor = highContrast
        ? palette.raisedSurface
        : palette.raisedSurface.withValues(
            alpha: Theme.of(context).brightness == Brightness.dark ? 0.92 : 0.9,
          );
    final borderColor = highContrast
        ? palette.border
        : palette.border.withValues(alpha: 0.84);
    final surfaceDecoration = BoxDecoration(
      color: surfaceColor,
      border: Border.all(color: borderColor),
      borderRadius: borderRadius,
    );
    final content = padding == null
        ? child
        : Padding(padding: padding!, child: child);

    if (highContrast) {
      return DecoratedBox(decoration: surfaceDecoration, child: content);
    }

    final isDark = Theme.of(context).brightness == Brightness.dark;
    final shadowDecoration = BoxDecoration(
      borderRadius: borderRadius,
      boxShadow: [
        BoxShadow(
          color: Colors.black.withValues(alpha: isDark ? 0.22 : 0.1),
          blurRadius: 18,
          offset: const Offset(0, 8),
        ),
      ],
    );

    return DecoratedBox(
      decoration: shadowDecoration,
      child: ClipRRect(
        borderRadius: borderRadius,
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
          child: DecoratedBox(decoration: surfaceDecoration, child: content),
        ),
      ),
    );
  }
}
