import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

ButtonStyle composerControlStyle(
  OpenChatPalette palette, {
  required double width,
  bool compact = false,
  double leftPadding = 10,
  double rightPadding = 10,
  Color? sideColor,
}) {
  final controlSide = sideColor ?? palette.border;
  return OutlinedButton.styleFrom(
    minimumSize: Size(width, 36),
    maximumSize: Size(width, 36),
    padding: EdgeInsets.fromLTRB(leftPadding, 0, rightPadding, 0),
    visualDensity: VisualDensity.standard,
    foregroundColor: palette.text,
    disabledForegroundColor: palette.disabledForeground,
    disabledBackgroundColor: palette.disabledSurface,
    tapTargetSize: compact
        ? MaterialTapTargetSize.padded
        : MaterialTapTargetSize.shrinkWrap,
    side: BorderSide(color: controlSide),
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(9)),
    textStyle: const TextStyle(
      fontSize: 13,
      fontWeight: FontWeight.w500,
      height: 18 / 13,
    ),
  ).copyWith(
    side: WidgetStateProperty.resolveWith<BorderSide?>((states) {
      final color = states.contains(WidgetState.disabled)
          ? palette.disabledBorder
          : controlSide;
      return BorderSide(color: color);
    }),
  );
}
