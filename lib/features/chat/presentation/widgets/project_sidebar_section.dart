import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_collapsible_heading.dart';

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
    this.onArchiveConversation,
    this.onEditConversationTags,
    this.onToggleConversationBookmark,
    this.selectionMode = false,
    this.selectedConversationIds = const <String>{},
    this.onToggleBatchSelection,
    this.collapsed = false,
    this.onToggleCollapsed,
    required this.onCreateProject,
    required this.loading,
    required this.errorMessage,
    this.onRetry,
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
  final void Function(String projectId, String projectRoot, String projectName)?
  onOpenProjectOptions;
  final ValueChanged<String>? onCreateProjectConversation;
  final ValueChanged<String>? onShowMoreProjectConversations;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;
  final ValueChanged<String>? onToggleConversationPinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final ValueChanged<String>? onArchiveConversation;
  final ValueChanged<String>? onEditConversationTags;
  final ValueChanged<String>? onToggleConversationBookmark;
  final bool selectionMode;
  final Set<String> selectedConversationIds;
  final ValueChanged<String>? onToggleBatchSelection;
  final bool collapsed;
  final VoidCallback? onToggleCollapsed;
  final VoidCallback? onCreateProject;
  final bool loading;
  final String? errorMessage;
  final VoidCallback? onRetry;
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
          alwaysShowAction: true,
          collapsed: collapsed,
          onToggleCollapsed: onToggleCollapsed,
        ),
        AnimatedSize(
          duration: MediaQuery.disableAnimationsOf(context)
              ? Duration.zero
              : const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topCenter,
          child: ClipRect(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (!collapsed) ...[
                  if (loading)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(4, 7, 4, 4),
                      child: Semantics(
                        liveRegion: true,
                        label: context.openchatL10n.projectLoading,
                        child: const LinearProgressIndicator(minHeight: 2),
                      ),
                    )
                  else if (errorMessage != null)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(0, 6, 0, 2),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              errorMessage!,
                              style: Theme.of(context).textTheme.bodyMedium
                                  ?.copyWith(
                                    color: palette.secondaryText,
                                    fontSize: OpenChatTypography.metadata,
                                    height: 18 / OpenChatTypography.metadata,
                                  ),
                            ),
                          ),
                          if (onRetry != null)
                            IconButton(
                              tooltip: l10n.retry,
                              onPressed: onRetry,
                              icon: const Icon(LucideIcons.refreshCw),
                            ),
                        ],
                      ),
                    )
                  else if (projects.isEmpty)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(0, 6, 0, 2),
                      child: Text(
                        emptyMessage,
                        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                          color: palette.secondaryText,
                          fontSize: 12,
                          height: 18 / 12,
                        ),
                      ),
                    ),
                  const SizedBox(height: 2),
                  for (final entry in projects) ...[
                    _ProjectSidebarTile(
                      project: entry.project,
                      selected: entry.project.id == selectedProjectId,
                      onSelect: onSelectProject == null
                          ? null
                          : () => onSelectProject!(entry.project.id),
                      onOpenOptions: onOpenProjectOptions == null
                          ? null
                          : () => onOpenProjectOptions!(
                              entry.project.id,
                              entry.project.folderPath,
                              entry.project.title,
                            ),
                      onCreateConversation: onCreateProjectConversation == null
                          ? null
                          : () =>
                                onCreateProjectConversation!(entry.project.id),
                      onMoveConversationToProject: onMoveConversationToProject,
                      key: ValueKey<String>(
                        'sidebar-project-${entry.project.id}',
                      ),
                    ),
                    if (entry.conversations.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 4),
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const SizedBox(width: 12),
                            Expanded(
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 4,
                                ),
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    for (
                                      var index = 0;
                                      index < entry.conversations.length;
                                      index++
                                    ) ...[
                                      if (index > 0) const SizedBox(height: 2),
                                      DraggableSidebarConversation(
                                        conversation:
                                            entry.conversations[index],
                                        child: SidebarConversationTile(
                                          conversation:
                                              entry.conversations[index],
                                          selected:
                                              entry.conversations[index].id ==
                                              selectedConversationId,
                                          selectionMode: selectionMode,
                                          selectedForBatch:
                                              selectedConversationIds.contains(
                                                entry.conversations[index].id,
                                              ),
                                          height: 32,
                                          inset: 8,
                                          showChatIcon: false,
                                          onPressed: selectionMode
                                              ? () => onToggleBatchSelection
                                                    ?.call(
                                                      entry
                                                          .conversations[index]
                                                          .id,
                                                    )
                                              : onSelectConversation == null
                                              ? null
                                              : () => onSelectConversation!(
                                                  entry.conversations[index].id,
                                                ),
                                          onTogglePinned:
                                              onToggleConversationPinned == null
                                              ? null
                                              : () =>
                                                    onToggleConversationPinned!(
                                                      entry
                                                          .conversations[index]
                                                          .id,
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
                                          onArchive:
                                              onArchiveConversation == null
                                              ? null
                                              : () => onArchiveConversation!(
                                                  entry.conversations[index].id,
                                                ),
                                          onEditTags:
                                              onEditConversationTags == null
                                              ? null
                                              : () => onEditConversationTags!(
                                                  entry.conversations[index].id,
                                                ),
                                          onToggleBookmark:
                                              onToggleConversationBookmark ==
                                                  null
                                              ? null
                                              : () =>
                                                    onToggleConversationBookmark!(
                                                      entry
                                                          .conversations[index]
                                                          .id,
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
                                        onPressed:
                                            onShowMoreProjectConversations ==
                                                null
                                            ? null
                                            : () =>
                                                  onShowMoreProjectConversations!(
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
                    if (entry != projects.last) const SizedBox(height: 12),
                  ],
                ],
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _ProjectsHeading extends StatefulWidget {
  const _ProjectsHeading({
    required this.title,
    required this.createProjectLabel,
    required this.onCreateProject,
    required this.alwaysShowAction,
    required this.collapsed,
    required this.onToggleCollapsed,
  });

  final String title;
  final String createProjectLabel;
  final VoidCallback? onCreateProject;
  final bool alwaysShowAction;
  final bool collapsed;
  final VoidCallback? onToggleCollapsed;

  @override
  State<_ProjectsHeading> createState() => _ProjectsHeadingState();
}

class _ProjectsHeadingState extends State<_ProjectsHeading> {
  late final FocusNode _createProjectFocusNode = FocusNode(
    debugLabel: 'create project',
  );
  bool _hovered = false;
  bool _focused = false;

  @override
  void dispose() {
    _createProjectFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => true,
      _ => false,
    };
    final showAction = widget.alwaysShowAction || touch || _hovered || _focused;
    final actionSize = touch ? 44.0 : 32.0;
    final duration = MediaQuery.disableAnimationsOf(context) || _focused
        ? Duration.zero
        : const Duration(milliseconds: 140);

    return Focus(
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: SidebarCollapsibleHeading(
          title: widget.title,
          collapsed: widget.collapsed,
          onPressed: widget.onToggleCollapsed ?? () {},
          trailing: widget.onCreateProject == null
              ? null
              : ExcludeSemantics(
                  excluding: !showAction,
                  child: ExcludeFocus(
                    excluding: !showAction,
                    child: IgnorePointer(
                      ignoring: !showAction,
                      child: AnimatedOpacity(
                        duration: duration,
                        opacity: showAction ? 1 : 0,
                        child: IconButton(
                          key: const ValueKey<String>('project-create-button'),
                          focusNode: _createProjectFocusNode,
                          tooltip: widget.createProjectLabel,
                          onPressed: widget.onCreateProject,
                          visualDensity: VisualDensity.standard,
                          constraints: BoxConstraints.tightFor(
                            width: actionSize,
                            height: actionSize,
                          ),
                          padding: EdgeInsets.zero,
                          icon: const Icon(LucideIcons.plus, size: 19),
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
      fontWeight: selected ? FontWeight.w500 : FontWeight.w400,
      height: 18 / 13,
    );

    return DragTarget<String>(
      onWillAcceptWithDetails: (_) => onMoveConversationToProject != null,
      onAcceptWithDetails: (details) =>
          onMoveConversationToProject?.call(details.data, project.id),
      builder: (context, candidates, _) => Focus(
        child: Builder(
          builder: (context) {
            final focused = Focus.of(context).hasFocus;
            return Semantics(
              button: true,
              enabled: onSelect != null,
              selected: selected,
              label: project.title,
              onTap: onSelect,
              child: Material(
                color: selected || candidates.isNotEmpty
                    ? palette.selected
                    : Colors.transparent,
                shape: RoundedRectangleBorder(
                  side: focused
                      ? BorderSide(color: palette.focusRing, width: 2)
                      : BorderSide.none,
                  borderRadius: BorderRadius.circular(6),
                ),
                child: InkWell(
                  onTap: onSelect,
                  hoverColor: palette.selected,
                  borderRadius: BorderRadius.circular(6),
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(minHeight: 44),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      child: Row(
                        children: [
                          Icon(
                            LucideIcons.folder,
                            color: selected || candidates.isNotEmpty
                                ? palette.text
                                : palette.secondaryIcon,
                            size: 15,
                          ),
                          const SizedBox(width: 6),
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
                                LucideIcons.ellipsis,
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
                              child: Icon(
                                LucideIcons.messageSquarePlus,
                                color: palette.secondaryIcon,
                                size: 18,
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            );
          },
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
    final dimension = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => 44.0,
      _ => 40.0,
    };

    return SizedBox.square(
      dimension: dimension,
      child: IconButton(
        tooltip: label,
        onPressed: onPressed,
        padding: EdgeInsets.zero,
        iconSize: 18,
        constraints: BoxConstraints.tightFor(
          width: dimension,
          height: dimension,
        ),
        style: IconButton.styleFrom(
          minimumSize: Size.square(dimension),
          maximumSize: Size.square(dimension),
          padding: EdgeInsets.zero,
          tapTargetSize: dimension == 44
              ? MaterialTapTargetSize.padded
              : MaterialTapTargetSize.shrinkWrap,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(OpenChatRadii.control),
          ),
        ),
        icon: child,
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
  Widget build(BuildContext context) => TextButton(
    onPressed: onPressed,
    style: TextButton.styleFrom(
      minimumSize: const Size(40, 40),
      padding: const EdgeInsets.only(left: 4, right: 8),
      alignment: Alignment.centerLeft,
      foregroundColor: color,
    ),
    child: Text(label, style: const TextStyle(fontSize: 13, height: 18 / 13)),
  );
}
