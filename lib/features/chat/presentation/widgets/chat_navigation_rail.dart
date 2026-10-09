import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_brand_mark.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class ChatNavigationRail extends StatelessWidget {
  const ChatNavigationRail({
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
    const railContentWidth = OpenChatSpacing.navigationRailButtonWidth;
    final chatsSelected =
        !settingsSelected &&
        !workspacesSelected &&
        !outputsSelected &&
        !modelsSelected;

    return Container(
      width: OpenChatSpacing.compactRailWidth,
      decoration: BoxDecoration(
        color: OpenChatSemanticColors.of(context).background,
      ),
      child: SafeArea(
        child: LayoutBuilder(
          builder: (context, constraints) {
            final content = Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: OpenChatSpacing.navigationRailInset,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const SizedBox(height: 4),
                  if (showBrand) ...[
                    const Tooltip(message: 'OpenChat', child: _CompactBrand()),
                    const SizedBox(height: 24),
                  ] else
                    const SizedBox(height: 4),
                  const SizedBox(height: 6),
                  Align(
                    alignment: Alignment.center,
                    child: SizedBox(
                      width: railContentWidth,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          _RailNavigationButton(
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
                              palette: palette,
                              onPressed: onToggleTheme,
                            ),
                          ),
                          const SizedBox(height: 8),
                          _RailNavigationButton(
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
    required this.label,
    required this.selected,
    required this.icon,
    required this.palette,
    this.onPressed,
  });

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
          expands: false,
          padding: EdgeInsets.zero,
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
            width: OpenChatSpacing.navigationRailButtonWidth,
            height: 44,
            child: Center(child: icon),
          ),
        ),
      ),
    );

    return Tooltip(message: label, child: item);
  }
}

class _ThemeButton extends StatelessWidget {
  const _ThemeButton({required this.palette, required this.onPressed});

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
          alignment: Alignment.center,
          foregroundColor: palette.secondaryText,
          padding: EdgeInsets.zero,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(OpenChatRadii.control),
          ),
        ),
        child: Icon(
          Theme.of(context).brightness == Brightness.dark
              ? LucideIcons.sun
              : LucideIcons.moon,
          size: 20,
          color: palette.secondaryIcon,
        ),
      ),
    );
  }
}
