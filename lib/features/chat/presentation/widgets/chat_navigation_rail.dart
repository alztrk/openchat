import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_brand_mark.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatNavigationRail extends StatelessWidget {
  const ChatNavigationRail({
    required this.expanded,
    required this.settingsSelected,
    this.modelsSelected = false,
    this.localModelsSelected = false,
    this.showBrand = true,
    required this.onOpenChat,
    this.onOpenModels,
    this.onOpenLocalModels,
    required this.onOpenSettings,
    required this.onToggleTheme,
    this.sidebarsCompact = false,
    this.onToggleSidebars,
    super.key,
  });

  final bool expanded;
  final bool settingsSelected;
  final bool modelsSelected;
  final bool localModelsSelected;
  final bool showBrand;
  final VoidCallback onOpenChat;
  final VoidCallback? onOpenModels;
  final VoidCallback? onOpenLocalModels;
  final VoidCallback onOpenSettings;
  final VoidCallback onToggleTheme;
  final bool sidebarsCompact;
  final VoidCallback? onToggleSidebars;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android || TargetPlatform.iOS => true,
      _ => false,
    };
    final railContentWidth = expanded
        ? 226.0
        : touch
        ? 44.0
        : 36.0;

    return Container(
      width: expanded
          ? OpenChatSpacing.expandedRailWidth
          : OpenChatSpacing.compactRailWidth,
      decoration: BoxDecoration(
        color: OpenChatSemanticColors.of(context).background,
      ),
      child: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final content = Padding(
              padding: EdgeInsets.symmetric(horizontal: expanded ? 16 : 4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(height: expanded ? 20 : 4),
                  if (expanded)
                    _Brand(palette: palette)
                  else if (showBrand) ...[
                    const _CompactBrand(),
                    const SizedBox(height: 24),
                  ] else
                    const SizedBox(height: 4),
                  SizedBox(height: expanded ? 10 : 6),
                  Align(
                    alignment: Alignment.center,
                    child: SizedBox(
                      width: railContentWidth,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          _RailNavigationButton(
                            expanded: expanded,
                            label: l10n.home,
                            selected:
                                !settingsSelected &&
                                !modelsSelected &&
                                !localModelsSelected,
                            icon: _RailIcon(
                              assetPath: 'assets/icons/home.svg',
                              color:
                                  settingsSelected ||
                                      modelsSelected ||
                                      localModelsSelected
                                  ? palette.secondaryIcon
                                  : palette.text,
                            ),
                            palette: palette,
                            onPressed: onOpenChat,
                          ),
                          if (onOpenModels case final openModels?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: l10n.models,
                              selected: modelsSelected,
                              icon: Icon(
                                Icons.hub_outlined,
                                color: modelsSelected
                                    ? palette.text
                                    : palette.secondaryIcon,
                                size: 19,
                              ),
                              palette: palette,
                              onPressed: openModels,
                            ),
                          ],
                          if (onOpenLocalModels
                              case final openLocalModels?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: l10n.localModelsPageTitle,
                              selected: localModelsSelected,
                              icon: Icon(
                                Icons.storage_rounded,
                                color: localModelsSelected
                                    ? palette.text
                                    : palette.secondaryIcon,
                                size: 19,
                              ),
                              palette: palette,
                              onPressed: openLocalModels,
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                  const Spacer(),
                  Align(
                    alignment: Alignment.center,
                    child: SizedBox(
                      width: railContentWidth,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Divider(
                            height: 1,
                            color: palette.border.withValues(alpha: 0.48),
                          ),
                          const SizedBox(height: 8),
                          Tooltip(
                            message: dark
                                ? l10n.switchToLightMode
                                : l10n.switchToDarkMode,
                            child: _ThemeButton(
                              expanded: expanded,
                              label: dark
                                  ? l10n.switchToLightMode
                                  : l10n.switchToDarkMode,
                              palette: palette,
                              onPressed: onToggleTheme,
                            ),
                          ),
                          const SizedBox(height: 8),
                          _RailNavigationButton(
                            expanded: expanded,
                            label: l10n.settings,
                            selected: settingsSelected,
                            icon: _RailIcon(
                              assetPath: 'assets/icons/settings.svg',
                              color: settingsSelected
                                  ? palette.text
                                  : palette.secondaryIcon,
                            ),
                            palette: palette,
                            onPressed: onOpenSettings,
                          ),
                          if (onToggleSidebars case final toggleSidebars?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: sidebarsCompact
                                  ? l10n.showSidebars
                                  : l10n.collapseSidebars,
                              selected: false,
                              icon: Icon(
                                sidebarsCompact
                                    ? Icons.chevron_right_rounded
                                    : Icons.chevron_left_rounded,
                                color: palette.secondaryIcon,
                                size: 20,
                              ),
                              palette: palette,
                              onPressed: toggleSidebars,
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),
                ],
              ),
            );
            if (constraints.maxHeight >= 480) return content;
            return SingleChildScrollView(
              child: SizedBox(height: 480, child: content),
            );
          },
        ),
      ),
    );
  }
}

class _Brand extends StatelessWidget {
  const _Brand({required this.palette});

  final OpenChatPalette palette;

  @override
  Widget build(BuildContext context) {
    const mark = OpenChatBrandMark(size: 58.8);

    return Semantics(
      label: 'OpenChat',
      child: SizedBox(
        height: 68,
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            Positioned(left: 50.78, top: 4.52, child: mark),
            Positioned(
              left: 115.2,
              top: 25.3,
              child: ExcludeSemantics(
                child: Text(
                  'OpenChat',
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 17.6,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 0.112,
                    height: 1.2,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CompactBrand extends StatelessWidget {
  const _CompactBrand();

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      key: const ValueKey<String>('compact-brand'),
      borderRadius: BorderRadius.circular(8),
      child: const OpenChatBrandMark(size: 44),
    );
  }
}

class _RailIcon extends StatelessWidget {
  const _RailIcon({required this.assetPath, required this.color});

  final String assetPath;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 20,
      height: 20,
      child: Center(
        child: SvgPicture.asset(
          assetPath,
          colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
          excludeFromSemantics: true,
        ),
      ),
    );
  }
}

class _RailNavigationButton extends StatelessWidget {
  const _RailNavigationButton({
    required this.expanded,
    required this.label,
    required this.selected,
    required this.icon,
    required this.palette,
    this.onPressed,
  });

  final bool expanded;
  final String label;
  final bool selected;
  final Widget icon;
  final OpenChatPalette palette;
  final VoidCallback? onPressed;

  bool get enabled => onPressed != null;

  @override
  Widget build(BuildContext context) {
    final foreground = selected ? palette.text : palette.secondaryText;
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android || TargetPlatform.iOS => true,
      _ => false,
    };
    final item = Semantics(
      button: true,
      enabled: enabled,
      selected: selected,
      label: label,
      child: ExcludeSemantics(
        child: Material(
          color: selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(OpenChatRadii.control),
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(OpenChatRadii.control),
            hoverColor: palette.hover,
            focusColor: OpenChatSemanticColors.of(context).focusRing
                .withValues(alpha: 0.24),
            child: SizedBox(
              height: touch || expanded ? 44 : 36,
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: expanded ? 12 : 0),
                child: Row(
                  mainAxisAlignment: expanded
                      ? MainAxisAlignment.start
                      : MainAxisAlignment.center,
                  children: [
                    icon,
                    if (expanded) ...[
                      const SizedBox(width: 12),
                      Expanded(
                        child: Text(
                          label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.bodyMedium
                              ?.copyWith(
                                color: foreground,
                                fontSize: 14,
                                height: 20 / 14,
                              ),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );

    return expanded ? item : Tooltip(message: label, child: item);
  }
}

class _ThemeButton extends StatelessWidget {
  const _ThemeButton({
    required this.expanded,
    required this.label,
    required this.palette,
    required this.onPressed,
  });

  final bool expanded;
  final String label;
  final OpenChatPalette palette;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android || TargetPlatform.iOS => true,
      _ => false,
    };
    return SizedBox(
      width: double.infinity,
      height: touch || expanded ? 44 : 36,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          alignment: expanded ? Alignment.centerLeft : Alignment.center,
          foregroundColor: palette.secondaryText,
          padding: EdgeInsets.symmetric(horizontal: expanded ? 12 : 0),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
        child: Row(
          mainAxisAlignment: expanded
              ? MainAxisAlignment.start
              : MainAxisAlignment.center,
          children: [
            Icon(
              Theme.of(context).brightness == Brightness.dark
                  ? Icons.light_mode_outlined
                  : Icons.dark_mode_outlined,
              size: 20,
              color: palette.secondaryIcon,
            ),
            if (expanded) ...[
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodyMedium
                      ?.copyWith(fontSize: 14, height: 20 / 14),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
