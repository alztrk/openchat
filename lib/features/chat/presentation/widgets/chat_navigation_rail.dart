import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_brand_mark.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatNavigationRail extends StatelessWidget {
  const ChatNavigationRail({
    required this.expanded,
    required this.settingsSelected,
    this.workspacesSelected = false,
    this.outputsSelected = false,
    this.modelsSelected = false,
    this.showBrand = true,
    required this.onOpenChat,
    this.onOpenWorkspaces,
    this.onOpenOutputs,
    this.onOpenModels,
    required this.onOpenSettings,
    required this.onToggleTheme,
    this.sidebarsCompact = false,
    this.onToggleSidebars,
    super.key,
  });

  final bool expanded;
  final bool settingsSelected;
  final bool workspacesSelected;
  final bool outputsSelected;
  final bool modelsSelected;
  final bool showBrand;
  final VoidCallback onOpenChat;
  final VoidCallback? onOpenWorkspaces;
  final VoidCallback? onOpenOutputs;
  final VoidCallback? onOpenModels;
  final VoidCallback onOpenSettings;
  final VoidCallback onToggleTheme;
  final bool sidebarsCompact;
  final VoidCallback? onToggleSidebars;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final railContentWidth = expanded ? 226.0 : 44.0;
    final chatsSelected =
        !settingsSelected &&
        !workspacesSelected &&
        !outputsSelected &&
        !modelsSelected;

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
                  if (showBrand) ...[
                    if (expanded)
                      _Brand(palette: palette)
                    else ...[
                      const _CompactBrand(),
                      const SizedBox(height: 24),
                    ],
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
                            label: l10n.chats,
                            selected: chatsSelected,
                            icon: Icon(
                              LucideIcons.messageCircle,
                              color: chatsSelected
                                  ? palette.accentIcon
                                  : palette.secondaryIcon,
                              size: 20,
                            ),
                            palette: palette,
                            onPressed: onOpenChat,
                          ),
                          if (onOpenWorkspaces case final openWorkspaces?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: l10n.workspaces,
                              selected: workspacesSelected,
                              icon: Icon(
                                LucideIcons.folder,
                                color: workspacesSelected
                                    ? palette.accentIcon
                                    : palette.secondaryIcon,
                                size: 20,
                              ),
                              palette: palette,
                              onPressed: openWorkspaces,
                            ),
                          ],
                          if (onOpenOutputs case final openOutputs?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: l10n.outputs,
                              selected: outputsSelected,
                              icon: Icon(
                                LucideIcons.bookmark,
                                color: outputsSelected
                                    ? palette.accentIcon
                                    : palette.secondaryIcon,
                                size: 20,
                              ),
                              palette: palette,
                              onPressed: openOutputs,
                            ),
                          ],
                          if (onOpenModels case final openModels?) ...[
                            const SizedBox(height: 8),
                            _RailNavigationButton(
                              expanded: expanded,
                              label: l10n.modelLibrary,
                              selected: modelsSelected,
                              icon: Icon(
                                LucideIcons.brain,
                                color: modelsSelected
                                    ? palette.accentIcon
                                    : palette.secondaryIcon,
                                size: 19,
                              ),
                              palette: palette,
                              onPressed: openModels,
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
                            icon: Icon(
                              LucideIcons.settings,
                              color: settingsSelected
                                  ? palette.accentIcon
                                  : palette.secondaryIcon,
                              size: 20,
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
                                    ? LucideIcons.chevronRight
                                    : LucideIcons.chevronLeft,
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
                    fontSize: OpenChatTypography.componentTitle,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 0,
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
      borderRadius: BorderRadius.circular(OpenChatRadii.control),
      child: const OpenChatBrandMark(size: 44),
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

  @override
  Widget build(BuildContext context) {
    final foreground = selected ? palette.text : palette.secondaryText;
    final item = Semantics(
      button: true,
      enabled: onPressed != null,
      selected: selected,
      label: label,
      child: ExcludeSemantics(
        child: shad.ShadButton.ghost(
          width: double.infinity,
          height: selected ? 46 : 44,
          expands: expanded,
          padding: EdgeInsets.symmetric(horizontal: expanded ? 12 : 0),
          enabled: onPressed != null,
          onPressed: onPressed,
          backgroundColor: selected ? palette.selected : Colors.transparent,
          hoverBackgroundColor: palette.hover,
          foregroundColor: foreground,
          decoration: shad.ShadDecoration(
            border: shad.ShadBorder.all(
              color: selected ? palette.border : Colors.transparent,
              radius: BorderRadius.circular(OpenChatRadii.control),
            ),
            focusedBorder: shad.ShadBorder.all(
              color: palette.focusRing,
              width: 2,
              radius: BorderRadius.circular(OpenChatRadii.control),
            ),
          ),
          child: SizedBox(
            height: 44,
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
                      style: Theme.of(context).textTheme.bodyMedium?.copyWith(
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
    return SizedBox(
      width: double.infinity,
      height: 44,
      child: TextButton(
        onPressed: onPressed,
        style: TextButton.styleFrom(
          alignment: expanded ? Alignment.centerLeft : Alignment.center,
          foregroundColor: palette.secondaryText,
          padding: EdgeInsets.symmetric(horizontal: expanded ? 12 : 0),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(OpenChatRadii.control),
          ),
        ),
        child: Row(
          mainAxisAlignment: expanded
              ? MainAxisAlignment.start
              : MainAxisAlignment.center,
          children: [
            Icon(
              Theme.of(context).brightness == Brightness.dark
                  ? LucideIcons.sun
                  : LucideIcons.moon,
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
