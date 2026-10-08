import 'package:flutter/material.dart';

import 'package:openchat/app/openchat_theme.dart';

class OpenChatPageHeader extends StatelessWidget {
  const OpenChatPageHeader({
    required this.title,
    this.description,
    this.actions = const <Widget>[],
    this.focusNode,
    super.key,
  });

  final String title;
  final String? description;
  final List<Widget> actions;
  final FocusNode? focusNode;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final description = this.description;
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
              Focus(
                focusNode: focusNode,
                child: Builder(
                  builder: (context) {
                    final focused = Focus.of(context).hasFocus;
                    return Semantics(
                      container: true,
                      header: true,
                      namesRoute: true,
                      liveRegion: true,
                      focusable: true,
                      focused: focused,
                      label: title,
                      child: ExcludeSemantics(
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            border: focused
                                ? Border.all(
                                    color: OpenChatPalette.of(context).focusRing,
                                    width: 2,
                                  )
                                : null,
                            borderRadius: BorderRadius.circular(
                              OpenChatRadii.control,
                            ),
                          ),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 3,
                              vertical: 2,
                            ),
                            child: Text(
                              title,
                              style: textTheme.headlineSmall,
                            ),
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ),
              if (description != null && description.trim().isNotEmpty) ...[
                const SizedBox(height: OpenChatSpacing.xs),
                Text(description, style: textTheme.bodyMedium),
              ],
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
