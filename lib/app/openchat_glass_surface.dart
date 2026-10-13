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
    this.surfaceColor,
    this.surfaceOpacity = 0.9,
    this.borderColor,
    this.borderWidth = 1,
    this.duration = Duration.zero,
    this.padding,
    super.key,
  });

  final Widget child;
  final BorderRadius borderRadius;
  final Color? surfaceColor;
  final double surfaceOpacity;
  final Color? borderColor;
  final double borderWidth;
  final Duration duration;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final highContrast = MediaQuery.highContrastOf(context);
    final baseSurfaceColor = surfaceColor ?? palette.raisedSurface;
    final resolvedSurfaceColor = highContrast
        ? baseSurfaceColor
        : baseSurfaceColor.withValues(alpha: surfaceOpacity);
    final baseBorderColor = borderColor ?? palette.border;
    final resolvedBorderColor = highContrast || borderColor != null
        ? baseBorderColor
        : baseBorderColor.withValues(alpha: 0.84);
    final surfaceDecoration = BoxDecoration(
      color: resolvedSurfaceColor,
      border: Border.all(color: resolvedBorderColor, width: borderWidth),
      borderRadius: borderRadius,
    );
    final content = padding == null
        ? child
        : Padding(padding: padding!, child: child);

    if (highContrast) {
      return AnimatedContainer(
        duration: duration,
        curve: Curves.easeOutCubic,
        decoration: surfaceDecoration,
        child: content,
      );
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
          child: AnimatedContainer(
            duration: duration,
            curve: Curves.easeOutCubic,
            decoration: surfaceDecoration,
            child: content,
          ),
        ),
      ),
    );
  }
}
