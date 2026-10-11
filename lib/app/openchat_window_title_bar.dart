import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_brand_mark.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/window_control_bar.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/platform/windows/window_controls.dart';

class OpenChatWindowTitleBar extends StatelessWidget {
  const OpenChatWindowTitleBar({
    required this.canGoBack,
    required this.canGoForward,
    required this.sidebarsCompact,
    required this.onGoBack,
    required this.onGoForward,
    required this.onToggleSidebars,
    super.key,
  });

  final bool canGoBack;
  final bool canGoForward;
  final bool sidebarsCompact;
  final VoidCallback onGoBack;
  final VoidCallback onGoForward;
  final VoidCallback onToggleSidebars;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    if (!OpenChatWindowControls.isSupportedOn(theme.platform)) {
      return const SizedBox.shrink();
    }

    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);

    return DecoratedBox(
      decoration: BoxDecoration(
        color: OpenChatSemanticColors.of(context).background,
      ),
      child: SizedBox(
        height: OpenChatSpacing.appTitleBarHeight,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final showNavigationControls = constraints.maxWidth >= 240;
              final showIdentity = constraints.maxWidth >= 280;
              final showBrandName =
                  showIdentity &&
                  constraints.maxWidth >= 360 &&
                  MediaQuery.textScalerOf(context).scale(14) <= 20;
              final showWindowControls = constraints.maxWidth >= 128;
              return Row(
                children: [
                  if (showNavigationControls) ...[
                    const SizedBox(width: 8),
                    _TitleBarActionButton(
                      label: l10n.previousPage,
                      icon: LucideIcons.arrowLeft,
                      onPressed: canGoBack ? onGoBack : null,
                      color: palette.secondaryIcon,
                    ),
                    const SizedBox(width: 4),
                    _TitleBarActionButton(
                      label: l10n.nextPage,
                      icon: LucideIcons.arrowRight,
                      onPressed: canGoForward ? onGoForward : null,
                      color: palette.secondaryIcon,
                    ),
                    const SizedBox(width: 4),
                    _TitleBarActionButton(
                      label: sidebarsCompact
                          ? l10n.showSidebars
                          : l10n.collapseSidebars,
                      icon: sidebarsCompact
                          ? LucideIcons.panelLeftOpen
                          : LucideIcons.panelLeftClose,
                      onPressed: onToggleSidebars,
                      color: palette.secondaryIcon,
                    ),
                  ],
                  if (showIdentity) ...[
                    SizedBox(width: showNavigationControls ? 8 : 12),
                    ExcludeSemantics(child: OpenChatBrandMark(size: 24)),
                    if (showBrandName) ...[
                      const SizedBox(width: 9),
                      Text(
                        'OpenChat',
                        style: theme.textTheme.bodyMedium?.copyWith(
                          color: palette.text,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ] else if (!showNavigationControls)
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
      ),
    );
  }
}

class _TitleBarActionButton extends StatelessWidget {
  const _TitleBarActionButton({
    required this.label,
    required this.icon,
    required this.onPressed,
    required this.color,
  });

  final String label;
  final IconData icon;
  final VoidCallback? onPressed;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      onPressed: onPressed,
      tooltip: label,
      constraints: const BoxConstraints.tightFor(width: 32, height: 32),
      padding: EdgeInsets.zero,
      visualDensity: VisualDensity.compact,
      color: color,
      disabledColor: color.withValues(alpha: 0.42),
      icon: Icon(icon, size: 18),
    );
  }
}
