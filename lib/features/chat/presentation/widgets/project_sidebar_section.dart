import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_section_heading.dart';

class ProjectSidebarSection extends StatelessWidget {
  const ProjectSidebarSection({
    super.key,
    required this.projects,
    required this.selectedProjectId,
    required this.selectedConversationId,
    required this.onSelectProject,
    required this.onSelectConversation,
    required this.onOpenProjectOptions,
    required this.onCreateProjectConversation,
    required this.onShowMoreProjectConversations,
    required this.onMoveConversationToProject,
    required this.onToggleConversationPinned,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onExportConversation,
    required this.onCreateProject,
    required this.loading,
    required this.errorMessage,
    required this.emptyMessage,
  });

  final List<
    ({
      ConversationSidebarProject project,
      List<ConversationSidebarConversation> conversations,
    })
  >
  projects;
  final String? selectedProjectId;
  final String? selectedConversationId;
  final ValueChanged<String>? onSelectProject;
  final ValueChanged<String>? onSelectConversation;
  final ValueChanged<String>? onOpenProjectOptions;
  final ValueChanged<String>? onCreateProjectConversation;
  final ValueChanged<String>? onShowMoreProjectConversations;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;
  final ValueChanged<String>? onToggleConversationPinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final VoidCallback? onCreateProject;
  final bool loading;
  final String? errorMessage;
  final String emptyMessage;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _ProjectsHeading(
          title: l10n.projects,
          createProjectLabel: l10n.createProject,
          onCreateProject: onCreateProject,
        ),
        if (loading)
          const Padding(
            padding: EdgeInsets.fromLTRB(4, 7, 4, 4),
            child: LinearProgressIndicator(minHeight: 2),
          )
        else if (errorMessage != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 6, 4, 2),
            child: Text(
              errorMessage!,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            ),
          )
        else if (projects.isEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 6, 4, 2),
            child: Text(
              emptyMessage,
              style: TextStyle(color: palette.secondaryText, fontSize: 12),
            ),
          ),
        const SizedBox(height: 4),
        for (final entry in projects) ...[
          _ProjectSidebarTile(
            project: entry.project,
            selected: entry.project.id == selectedProjectId,
            onSelect: onSelectProject == null
                ? null
                : () => onSelectProject!(entry.project.id),
            onOpenOptions: onOpenProjectOptions == null
                ? null
                : () => onOpenProjectOptions!(entry.project.id),
            onCreateConversation: onCreateProjectConversation == null
                ? null
                : () => onCreateProjectConversation!(entry.project.id),
            onMoveConversationToProject: onMoveConversationToProject,
            key: ValueKey<String>('sidebar-project-${entry.project.id}'),
          ),
          if (entry.conversations.isNotEmpty) ...[
            const SizedBox(height: 8),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SizedBox(
                    width: 24,
                    height:
                        entry.conversations.length * 40.0 +
                        (entry.conversations.length - 1) * 4.0,
                    child: CustomPaint(
                      painter: _ProjectConversationTreePainter(
                        itemCount: entry.conversations.length,
                        color:
                            Color.lerp(
                              palette.border,
                              palette.secondaryText,
                              0.18,
                            ) ??
                            palette.border,
                      ),
                    ),
                  ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          for (
                            var index = 0;
                            index < entry.conversations.length;
                            index++
                          ) ...[
                            if (index > 0) const SizedBox(height: 4),
                            DraggableSidebarConversation(
                              conversation: entry.conversations[index],
                              child: SidebarConversationTile(
                                conversation: entry.conversations[index],
                                selected:
                                    entry.conversations[index].id ==
                                    selectedConversationId,
                                height: 40,
                                inset: 8,
                                showChatIcon: true,
                                onPressed: onSelectConversation == null
                                    ? null
                                    : () => onSelectConversation!(
                                        entry.conversations[index].id,
                                      ),
                                onTogglePinned:
                                    onToggleConversationPinned == null
                                    ? null
                                    : () => onToggleConversationPinned!(
                                        entry.conversations[index].id,
                                      ),
                                onRename: onRenameConversation == null
                                    ? null
                                    : () => onRenameConversation!(
                                        entry.conversations[index].id,
                                      ),
                                onDelete: onDeleteConversation == null
                                    ? null
                                    : () => onDeleteConversation!(
                                        entry.conversations[index].id,
                                      ),
                                onExport: onExportConversation == null
                                    ? null
                                    : () => onExportConversation!(
                                        entry.conversations[index].id,
                                      ),
                                key: ValueKey<String>(
                                  'sidebar-conversation-${entry.conversations[index].id}',
                                ),
                              ),
                            ),
                          ],
                          if (entry.project.hasMoreConversations) ...[
                            const SizedBox(height: 6),
                            _ShowMoreProjectConversations(
                              label: l10n.showMore,
                              color: palette.secondaryText,
                              onPressed: onShowMoreProjectConversations == null
                                  ? null
                                  : () => onShowMoreProjectConversations!(
                                      entry.project.id,
                                    ),
                            ),
                          ],
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          if (entry != projects.last) const SizedBox(height: 16),
        ],
      ],
    );
  }
}

class _ProjectsHeading extends StatefulWidget {
  const _ProjectsHeading({
    required this.title,
    required this.createProjectLabel,
    required this.onCreateProject,
  });

  final String title;
  final String createProjectLabel;
  final VoidCallback? onCreateProject;

  @override
  State<_ProjectsHeading> createState() => _ProjectsHeadingState();
}

class _ProjectsHeadingState extends State<_ProjectsHeading> {
  bool _hovered = false;
  bool _focused = false;

  bool get _showAction => _hovered || _focused;

  @override
  Widget build(BuildContext context) {
    final duration = MediaQuery.disableAnimationsOf(context)
        ? Duration.zero
        : const Duration(milliseconds: 140);

    return Focus(
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: SizedBox(
          height: 32,
          child: Row(
            children: [
              Expanded(child: SidebarSectionHeading(title: widget.title)),
              ExcludeSemantics(
                excluding: !_showAction,
                child: IgnorePointer(
                  ignoring: !_showAction,
                  child: AnimatedOpacity(
                    duration: duration,
                    opacity: _showAction ? 1 : 0,
                    child: IconButton(
                      key: const ValueKey<String>('project-create-button'),
                      tooltip: widget.createProjectLabel,
                      onPressed: widget.onCreateProject,
                      visualDensity: VisualDensity.compact,
                      constraints: const BoxConstraints.tightFor(
                        width: 40,
                        height: 40,
                      ),
                      padding: EdgeInsets.zero,
                      icon: const Icon(Icons.add_rounded, size: 19),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ProjectConversationTreePainter extends CustomPainter {
  const _ProjectConversationTreePainter({
    required this.itemCount,
    required this.color,
  });

  final int itemCount;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    if (itemCount == 0) return;

    final paint = Paint()
      ..color = color
      ..strokeWidth = 1.2
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;
    const trunkX = 8.0;
    const rowHeight = 40.0;
    const rowGap = 4.0;
    final lastCenterY = (itemCount - 1) * (rowHeight + rowGap) + rowHeight / 2;

    canvas.drawLine(Offset(trunkX, 0), Offset(trunkX, lastCenterY), paint);
    for (var index = 0; index < itemCount; index++) {
      final centerY = index * (rowHeight + rowGap) + rowHeight / 2;
      canvas.drawLine(
        Offset(trunkX, centerY),
        Offset(size.width - 1, centerY),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(_ProjectConversationTreePainter oldDelegate) =>
      itemCount != oldDelegate.itemCount || color != oldDelegate.color;
}

class _ProjectSidebarTile extends StatelessWidget {
  const _ProjectSidebarTile({
    required this.project,
    required this.selected,
    required this.onSelect,
    required this.onOpenOptions,
    required this.onCreateConversation,
    required this.onMoveConversationToProject,
    super.key,
  });

  final ConversationSidebarProject project;
  final bool selected;
  final VoidCallback? onSelect;
  final VoidCallback? onOpenOptions;
  final VoidCallback? onCreateConversation;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final labelStyle = Theme.of(context).textTheme.bodyMedium?.copyWith(
      color: selected ? palette.text : palette.secondaryText,
      fontSize: 13,
      fontWeight: selected ? FontWeight.w600 : FontWeight.w500,
      height: 20 / 13,
    );

    return DragTarget<String>(
      onWillAcceptWithDetails: (_) => onMoveConversationToProject != null,
      onAcceptWithDetails: (details) =>
          onMoveConversationToProject?.call(details.data, project.id),
      builder: (context, candidates, _) => Semantics(
        button: true,
        enabled: onSelect != null,
        selected: selected,
        label: project.title,
        child: Material(
          color: selected || candidates.isNotEmpty
              ? palette.selected
              : palette.hover,
          shape: RoundedRectangleBorder(
            side: BorderSide(color: selected ? palette.accent : palette.border),
            borderRadius: BorderRadius.circular(8),
          ),
          child: InkWell(
            onTap: onSelect,
            hoverColor: palette.selected,
            borderRadius: BorderRadius.circular(8),
            child: SizedBox(
              height: 38,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: Row(
                  children: [
                    Icon(
                      Icons.folder_outlined,
                      color: selected || candidates.isNotEmpty
                          ? palette.accentIcon
                          : palette.secondaryIcon,
                      size: 17,
                    ),
                    const SizedBox(width: 9),
                    Expanded(
                      child: Text(
                        project.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: labelStyle,
                        key: ValueKey<String>(
                          'sidebar-project-label-${project.id}',
                        ),
                      ),
                    ),
                    if (onOpenOptions != null)
                      _ProjectAction(
                        label: l10n.projectOptions,
                        onPressed: onOpenOptions,
                        key: ValueKey<String>(
                          'sidebar-project-options-${project.id}',
                        ),
                        child: Icon(
                          Icons.more_horiz_rounded,
                          color: palette.secondaryIcon,
                          size: 18,
                        ),
                      ),
                    if (onCreateConversation != null)
                      _ProjectAction(
                        label: l10n.newProjectConversation,
                        onPressed: onCreateConversation,
                        key: ValueKey<String>(
                          'sidebar-project-new-chat-${project.id}',
                        ),
                        child: SvgPicture.asset(
                          Theme.of(context).brightness == Brightness.dark
                              ? 'assets/icons/conversation/dark.svg'
                              : 'assets/icons/conversation/light.svg',
                          width: 18,
                          height: 18,
                          colorFilter: ColorFilter.mode(
                            palette.secondaryIcon,
                            BlendMode.srcIn,
                          ),
                          excludeFromSemantics: true,
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

class _ProjectAction extends StatelessWidget {
  const _ProjectAction({
    required this.label,
    required this.onPressed,
    required this.child,
    super.key,
  });

  final String label;
  final VoidCallback? onPressed;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      enabled: onPressed != null,
      label: label,
      child: ExcludeSemantics(
        child: Tooltip(
          message: label,
          child: InkWell(
            onTap: onPressed,
            borderRadius: BorderRadius.circular(6),
            child: SizedBox(width: 24, height: 28, child: Center(child: child)),
          ),
        ),
      ),
    );
  }
}

class _ShowMoreProjectConversations extends StatelessWidget {
  const _ShowMoreProjectConversations({
    required this.label,
    required this.color,
    required this.onPressed,
  });

  final String label;
  final Color color;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      enabled: onPressed != null,
      label: label,
      child: ExcludeSemantics(
        child: InkWell(
          onTap: onPressed,
          borderRadius: BorderRadius.circular(6),
          child: SizedBox(
            height: 18,
            child: Padding(
              padding: const EdgeInsets.only(left: 4),
              child: Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  label,
                  style: TextStyle(
                    color: color,
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                    height: 18 / 12,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
