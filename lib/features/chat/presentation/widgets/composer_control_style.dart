import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

double composerControlHeight(BuildContext context) {
  final minimum = switch (Theme.of(context).platform) {
    TargetPlatform.android ||
    TargetPlatform.iOS ||
    TargetPlatform.fuchsia => 44.0,
    _ => 36.0,
  };
  final scaledHeight = MediaQuery.textScalerOf(context).scale(18) + 24;
  return scaledHeight > minimum ? scaledHeight : minimum;
}

ButtonStyle composerControlStyle(
  OpenChatPalette palette, {
  required double width,
  required double height,
  bool compact = false,
  double leftPadding = 10,
  double rightPadding = 10,
  Color? sideColor,
  bool borderless = false,
}) {
  final controlSide = sideColor ?? palette.controlBorder;
  return OutlinedButton.styleFrom(
    minimumSize: Size(width, height),
    maximumSize: Size(width, double.infinity),
    padding: EdgeInsets.fromLTRB(leftPadding, 0, rightPadding, 0),
    visualDensity: VisualDensity.standard,
    foregroundColor: palette.text,
    disabledForegroundColor: palette.disabledForeground,
    disabledBackgroundColor: Colors.transparent,
    tapTargetSize: compact
        ? MaterialTapTargetSize.padded
        : MaterialTapTargetSize.shrinkWrap,
    side: borderless ? BorderSide.none : BorderSide(color: controlSide),
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(OpenChatRadii.control),
    ),
  ).copyWith(
    side: WidgetStateProperty.resolveWith<BorderSide?>((states) {
      if (borderless) return BorderSide.none;
      if (states.contains(WidgetState.disabled)) {
        return BorderSide(color: palette.disabledBorder);
      }
      return BorderSide(color: controlSide);
    }),
    overlayColor: WidgetStateProperty.resolveWith((states) {
      if (states.contains(WidgetState.disabled)) return Colors.transparent;
      if (states.contains(WidgetState.pressed)) {
        return palette.selected.withValues(alpha: 0.8);
      }
      if (states.contains(WidgetState.focused) ||
          states.contains(WidgetState.hovered)) {
        return palette.hover;
      }
      return Colors.transparent;
    }),
  );
}
