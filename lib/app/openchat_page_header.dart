import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

class OpenChatPageHeader extends StatelessWidget {
  const OpenChatPageHeader({
    required this.title,
    required this.description,
    this.actions = const <Widget>[],
    super.key,
  });

  final String title;
  final String description;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return Wrap(
      alignment: WrapAlignment.spaceBetween,
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: OpenChatSpacing.lg,
      runSpacing: OpenChatSpacing.md,
      children: [
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: textTheme.headlineSmall),
              const SizedBox(height: OpenChatSpacing.xs),
              Text(description, style: textTheme.bodyMedium),
            ],
          ),
        ),
        if (actions.isNotEmpty)
          Wrap(
            alignment: WrapAlignment.end,
            spacing: OpenChatSpacing.xs,
            runSpacing: OpenChatSpacing.xs,
            children: actions,
          ),
      ],
    );
  }
}
