import 'package:flutter/material.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_theme.dart';

Widget openChatShadTestScope(ThemeData theme, Widget child) {
  return shad.ShadTheme(data: OpenChatTheme.shadFromTheme(theme), child: child);
}

Widget openChatShadTestBuilder(BuildContext context, Widget? child) {
  final content = child ?? const SizedBox.shrink();
  final theme = Theme.of(context);
  if (theme.extension<OpenChatPalette>() == null) return content;
  return openChatShadTestScope(theme, content);
}
