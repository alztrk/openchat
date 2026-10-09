import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_section_heading.dart';

class SidebarCollapsibleHeading extends StatefulWidget {
  const SidebarCollapsibleHeading({
    required this.title,
    required this.collapsed,
    required this.onPressed,
    this.icon,
    this.trailing,
    super.key,
  });

  final String title;
  final bool collapsed;
  final VoidCallback onPressed;
  final IconData? icon;
  final Widget? trailing;

  @override
  State<SidebarCollapsibleHeading> createState() =>
      _SidebarCollapsibleHeadingState();
}

class _SidebarCollapsibleHeadingState extends State<SidebarCollapsibleHeading> {
  bool _focused = false;
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final reducedMotion = MediaQuery.disableAnimationsOf(context);

    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: AnimatedContainer(
        duration: reducedMotion
            ? Duration.zero
            : const Duration(milliseconds: 120),
        decoration: BoxDecoration(
          color: _hovered || _focused ? palette.hover : Colors.transparent,
          borderRadius: BorderRadius.circular(OpenChatRadii.button),
        ),
        child: Row(
          children: [
            Expanded(
              child: Focus(
                onFocusChange: (focused) => setState(() => _focused = focused),
                child: Semantics(
                  button: true,
                  enabled: true,
                  expanded: !widget.collapsed,
                  label: widget.title,
                  onTap: widget.onPressed,
                  child: ExcludeSemantics(
                    child: Material(
                      color: Colors.transparent,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(
                          OpenChatRadii.button,
                        ),
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: InkWell(
                        onTap: widget.onPressed,
                        hoverColor: Colors.transparent,
                        borderRadius: BorderRadius.circular(
                          OpenChatRadii.button,
                        ),
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(minHeight: 40),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 4),
                            child: Row(
                              children: [
                                if (widget.icon != null) ...[
                                  Icon(
                                    widget.icon,
                                    size: 16,
                                    color: palette.secondaryIcon,
                                  ),
                                  const SizedBox(width: 8),
                                ],
                                Expanded(
                                  child: SidebarSectionHeading(
                                    title: widget.title,
                                  ),
                                ),
                                AnimatedRotation(
                                  duration: reducedMotion
                                      ? Duration.zero
                                      : const Duration(milliseconds: 120),
                                  turns: widget.collapsed ? 0 : 0.5,
                                  child: Icon(
                                    LucideIcons.chevronDown,
                                    size: 16,
                                    color: palette.secondaryIcon,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
            if (widget.trailing != null) widget.trailing!,
          ],
        ),
      ),
    );
  }
}
