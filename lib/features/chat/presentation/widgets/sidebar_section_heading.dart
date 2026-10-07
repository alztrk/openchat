import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

class SidebarSectionHeading extends StatelessWidget {
  const SidebarSectionHeading({required this.title, super.key});

  final String title;

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);

    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: 18),
      child: Text(
        title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
          color: palette.secondaryText,
          fontSize: 12,
          fontWeight: FontWeight.w500,
          height: 18 / 12,
        ),
      ),
    );
  }
}
