import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

double composerControlHeight(BuildContext context) {
  final minimum = switch (Theme.of(context).platform) {
    TargetPlatform.android ||
    TargetPlatform.iOS ||
    TargetPlatform.fuchsia => 44.0,
    _ => 36.0,
  };
  final scaledHeight = MediaQuery.textScalerOf(context).scale(18) + 12;
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
  Color? focusColor,
}) {
  final controlSide = sideColor ?? Colors.transparent;
  final resolvedFocusColor = focusColor ?? palette.accent;
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
    side: BorderSide(color: controlSide),
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(OpenChatRadii.control),
    ),
  ).copyWith(
    side: WidgetStateProperty.resolveWith<BorderSide?>((states) {
      if (states.contains(WidgetState.disabled)) {
        return BorderSide(color: controlSide);
      }
      if (states.contains(WidgetState.focused)) {
        return BorderSide(color: resolvedFocusColor, width: 1.4);
      }
      return BorderSide(color: controlSide);
    }),
  );
}
