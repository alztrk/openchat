import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/zihora_theme.dart';
import '../../../../l10n/zihora_localizations.dart';

class ChatNavigationRail extends StatelessWidget {
  const ChatNavigationRail({
    required this.expanded,
    required this.settingsSelected,
    required this.onOpenChat,
    required this.onOpenSettings,
    required this.onToggleTheme,
    this.onCollapseSidebars,
    super.key,
  });

  final bool expanded;
  final bool settingsSelected;
  final VoidCallback onOpenChat;
  final VoidCallback onOpenSettings;
  final VoidCallback onToggleTheme;
  final VoidCallback? onCollapseSidebars;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final railContentWidth = expanded ? 226.0 : 44.0;

    return Container(
      width: expanded
          ? ZihoraSpacing.expandedRailWidth
          : ZihoraSpacing.compactRailWidth,
      decoration: BoxDecoration(color: palette.navigation),
      foregroundDecoration: BoxDecoration(
        border: Border.all(color: palette.border),
      ),
      child: Padding(
        padding: const EdgeInsets.all(1),
        child: SafeArea(
          child: Padding(
            padding: EdgeInsets.symmetric(horizontal: expanded ? 16 : 13),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const SizedBox(height: 20),
                if (expanded)
                  Row(
                    children: [
                      Expanded(
                        child: _Brand(dark: dark, palette: palette),
                      ),
                      IconButton(
                        tooltip: l10n.collapseSidebars,
                        onPressed: onCollapseSidebars,
                        visualDensity: VisualDensity.compact,
                        constraints: const BoxConstraints.tightFor(
                          width: 32,
                          height: 32,
                        ),
                        padding: EdgeInsets.zero,
                        icon: Icon(
                          Icons.chevron_left_rounded,
                          color: palette.secondaryIcon,
                          size: 20,
                        ),
                      ),
                    ],
                  )
                else ...[
                  _CompactBrand(dark: dark),
                  const SizedBox(height: 8),
                  Tooltip(
                    message: l10n.collapseSidebars,
                    child: IconButton(
                      onPressed: onCollapseSidebars,
                      visualDensity: VisualDensity.compact,
                      constraints: const BoxConstraints.tightFor(
                        width: 44,
                        height: 32,
                      ),
                      padding: EdgeInsets.zero,
                      icon: Icon(
                        Icons.chevron_left_rounded,
                        color: palette.secondaryIcon,
                        size: 20,
                      ),
                    ),
                  ),
                ],
                const SizedBox(height: 10),
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
                          selected: !settingsSelected,
                          icon: _RailIcon(
                            assetPath: 'assets/icons/home.svg',
                            color: settingsSelected
                                ? palette.secondaryIcon
                                : palette.accentIcon,
                          ),
                          palette: palette,
                          onPressed: onOpenChat,
                        ),
                        const SizedBox(height: 6),
                        _RailNavigationButton(
                          expanded: expanded,
                          label: l10n.extensions,
                          selected: false,
                          icon: _RailIcon(
                            assetPath: 'assets/icons/wrench.svg',
                            color: palette.secondaryIcon,
                          ),
                          palette: palette,
                          unavailableHint: l10n.sectionUnavailable,
                        ),
                        const SizedBox(height: 6),
                        _RailNavigationButton(
                          expanded: expanded,
                          label: l10n.scheduled,
                          selected: false,
                          icon: _RailIcon(
                            assetPath: 'assets/icons/clock.svg',
                            color: palette.secondaryIcon,
                          ),
                          palette: palette,
                          unavailableHint: l10n.sectionUnavailable,
                        ),
                        const SizedBox(height: 6),
                        _RailNavigationButton(
                          expanded: expanded,
                          label: l10n.design,
                          selected: false,
                          icon: _RailIcon(
                            assetPath: 'assets/icons/pen-tool.svg',
                            color: palette.secondaryIcon,
                          ),
                          palette: palette,
                          unavailableHint: l10n.sectionUnavailable,
                        ),
                        const SizedBox(height: 6),
                        _RailNavigationButton(
                          expanded: expanded,
                          label: l10n.security,
                          selected: false,
                          icon: _RailIcon(
                            assetPath: 'assets/icons/shield.svg',
                            color: palette.secondaryIcon,
                          ),
                          palette: palette,
                          unavailableHint: l10n.sectionUnavailable,
                        ),
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
                        Divider(height: 1, color: palette.border),
                        const SizedBox(height: 8),
                        Tooltip(
                          message: dark
                              ? l10n.switchToLightMode
                              : l10n.switchToDarkMode,
                          child: _ThemeButton(
                            expanded: expanded,
                            label: l10n.theme,
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
                                ? palette.accentIcon
                                : palette.secondaryIcon,
                          ),
                          palette: palette,
                          onPressed: onOpenSettings,
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _Brand extends StatelessWidget {
  const _Brand({required this.dark, required this.palette});

  final bool dark;
  final ZihoraPalette palette;

  @override
  Widget build(BuildContext context) {
    final mark = _BrandMark(dark: dark, size: 58.8);

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
  const _CompactBrand({required this.dark});

  final bool dark;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      key: const ValueKey<String>('compact-brand'),
      borderRadius: BorderRadius.circular(8),
      child: _BrandMark(dark: dark, size: 44),
    );
  }
}

class _BrandMark extends StatelessWidget {
  const _BrandMark({required this.dark, required this.size});

  final bool dark;
  final double size;

  static const _sourceSize = 58.8;

  @override
  Widget build(BuildContext context) {
    final layerRoot = dark ? 'assets/brand/dark' : 'assets/brand/light';

    return SizedBox(
      width: size,
      height: size,
      child: FittedBox(
        fit: BoxFit.contain,
        child: ClipRect(
          child: SizedBox(
            width: _sourceSize,
            height: _sourceSize,
            child: Stack(
              children: [
                Positioned(
                  left: -11.76,
                  top: -11.76,
                  width: 81.87,
                  height: 81.87,
                  child: SvgPicture.asset('$layerRoot/base.svg'),
                ),
                const Positioned(
                  left: 5.08,
                  top: 8.74,
                  width: 22.67,
                  height: 42.91,
                  child: _BrandLayer(fileName: 'left.svg'),
                ),
                const Positioned(
                  left: 31.02,
                  top: 8.74,
                  width: 22.67,
                  height: 42.91,
                  child: _BrandLayer(fileName: 'right.svg'),
                ),
                const Positioned(
                  left: 22.95,
                  top: 3.98,
                  width: 12.86,
                  height: 36.6,
                  child: _BrandLayer(fileName: 'copper-core.svg'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _BrandLayer extends StatelessWidget {
  const _BrandLayer({required this.fileName});

  final String fileName;

  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final layerRoot = dark ? 'assets/brand/dark' : 'assets/brand/light';
    return SvgPicture.asset(
      '$layerRoot/$fileName',
      fit: BoxFit.fill,
      excludeFromSemantics: true,
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
    this.unavailableHint,
    this.onPressed,
  });

  final bool expanded;
  final String label;
  final bool selected;
  final Widget icon;
  final ZihoraPalette palette;
  final String? unavailableHint;
  final VoidCallback? onPressed;

  bool get enabled => onPressed != null;

  @override
  Widget build(BuildContext context) {
    final foreground = selected ? palette.text : palette.secondaryText;
    final item = Semantics(
      button: true,
      enabled: enabled,
      selected: selected,
      label: label,
      hint: unavailableHint,
      child: ExcludeSemantics(
        child: Material(
          color: selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(8),
            child: SizedBox(
              height: 44,
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
                      Text(
                        label,
                        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: foreground,
                          fontSize: 14,
                          height: 20 / 14,
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

    if (!expanded || unavailableHint != null) {
      final tooltip = unavailableHint == null
          ? label
          : '$label · $unavailableHint';
      return Tooltip(message: tooltip, child: item);
    }
    return item;
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
  final ZihoraPalette palette;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      height: 44,
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
            SvgPicture.asset(
              'assets/icons/sun.svg',
              width: 20,
              height: 20,
              colorFilter: ColorFilter.mode(
                palette.secondaryIcon,
                BlendMode.srcIn,
              ),
              excludeFromSemantics: true,
            ),
            if (expanded) ...[
              const SizedBox(width: 12),
              Text(
                label,
                style: Theme.of(context).textTheme.bodyMedium
                    ?.copyWith(fontSize: 14, height: 20 / 14),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
