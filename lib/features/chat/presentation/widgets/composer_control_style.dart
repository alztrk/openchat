import 'package:flutter/material.dart';

import '../../../../app/openchat_theme.dart';

ButtonStyle composerControlStyle(
  OpenChatPalette palette, {
  required double width,
  bool compact = false,
  double leftPadding = 10,
  double rightPadding = 10,
  Color? sideColor,
}) {
  return OutlinedButton.styleFrom(
    minimumSize: Size(width, 36),
    maximumSize: Size(width, 36),
    padding: EdgeInsets.fromLTRB(leftPadding, 0, rightPadding, 0),
    visualDensity: VisualDensity.standard,
    foregroundColor: palette.text,
    disabledForegroundColor: palette.secondaryText,
    tapTargetSize: compact
        ? MaterialTapTargetSize.padded
        : MaterialTapTargetSize.shrinkWrap,
    side: BorderSide(color: sideColor ?? palette.border),
    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(9)),
    textStyle: const TextStyle(
      fontFamily: 'Manrope',
      fontSize: 13,
      fontWeight: FontWeight.w500,
      height: 18 / 13,
    ),
  );
}
