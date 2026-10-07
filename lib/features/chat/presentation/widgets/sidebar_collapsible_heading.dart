import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_section_heading.dart';

class SidebarCollapsibleHeading extends StatefulWidget {
  const SidebarCollapsibleHeading({
    required this.title,
    required this.collapsed,
    required this.onPressed,
    this.trailing,
    super.key,
  });

  final String title;
  final bool collapsed;
  final VoidCallback onPressed;
  final Widget? trailing;

  @override
  State<SidebarCollapsibleHeading> createState() =>
      _SidebarCollapsibleHeadingState();
}

class _SidebarCollapsibleHeadingState extends State<SidebarCollapsibleHeading> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => true,
      _ => false,
    };
    final showArrow = touch || _hovered || _focused;

    return Focus(
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          child: InkWell(
            onTap: widget.onPressed,
            borderRadius: BorderRadius.circular(6),
            child: ConstrainedBox(
              constraints: const BoxConstraints(minHeight: 28),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Row(
                  children: [
                    Expanded(child: SidebarSectionHeading(title: widget.title)),
                    if (widget.trailing != null) widget.trailing!,
                    ExcludeSemantics(
                      excluding: !showArrow,
                      child: AnimatedOpacity(
                        duration: MediaQuery.disableAnimationsOf(context)
                            ? Duration.zero
                            : const Duration(milliseconds: 120),
                        opacity: showArrow ? 1 : 0,
                        child: Icon(
                          widget.collapsed
                              ? LucideIcons.chevronDown
                              : LucideIcons.chevronUp,
                          size: 15,
                          color: palette.secondaryIcon,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
