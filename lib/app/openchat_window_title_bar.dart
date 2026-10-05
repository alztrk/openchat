import 'dart:async';

import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_brand_mark.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/window_control_bar.dart';
import 'package:openchat/platform/windows/window_controls.dart';

class OpenChatWindowTitleBar extends StatelessWidget {
  const OpenChatWindowTitleBar({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    if (!OpenChatWindowControls.isSupportedOn(theme.platform)) {
      return const SizedBox.shrink();
    }

    final palette = OpenChatPalette.of(context);

    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.navigation,
        border: Border(
          bottom: BorderSide(color: palette.border.withValues(alpha: 0.48)),
        ),
      ),
      child: SizedBox(
        height: OpenChatSpacing.appTitleBarHeight,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final showIdentity = constraints.maxWidth >= 240;
            final showWindowControls = constraints.maxWidth >= 128;
            return Row(
              children: [
                if (showIdentity) ...[
                  const SizedBox(width: 12),
                  ExcludeSemantics(child: OpenChatBrandMark(size: 24)),
                  const SizedBox(width: 9),
                  Text(
                    'OpenChat',
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: palette.text,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ] else
                  SizedBox(width: constraints.maxWidth >= 8 ? 8 : 0),
                Expanded(
                  child: GestureDetector(
                    behavior: HitTestBehavior.translucent,
                    onPanStart: (_) =>
                        unawaited(OpenChatWindowControls.startDragging()),
                    child: const SizedBox.expand(),
                  ),
                ),
                if (showWindowControls) ...[
                  const WindowControlBar(),
                  if (constraints.maxWidth >= 136) const SizedBox(width: 8),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}
