import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../../app/zihora_theme.dart';
import '../../../../l10n/zihora_localizations.dart';
import '../../domain/conversation_sidebar_data.dart';

class ConversationSidebar extends StatelessWidget {
  const ConversationSidebar({
    required this.searchController,
    this.width = ZihoraSpacing.sidebarWidth,
    this.showDivider = true,
    this.isLoading = false,
    this.errorMessage,
    this.projects = const <ConversationSidebarProject>[],
    this.pinnedConversations = const <ConversationSidebarConversation>[],
    this.conversations = const <ConversationSidebarConversation>[],
    this.selectedProjectId,
    this.selectedConversationId,
    this.onSelectProject,
    this.onSelectConversation,
    this.onToggleConversationPinned,
    this.onRenameConversation,
    this.onDeleteConversation,
    this.onExportConversation,
    this.onMoveConversationToProject,
    this.onCreateProject,
    this.projectsLoading = false,
    this.projectLoadError,
    this.onOpenProjectOptions,
    this.onCreateProjectConversation,
    this.onShowMoreProjectConversations,
    this.onCreateConversation,
    this.onMoveConversationToChats,
    this.onPinConversation,
    super.key,
  });

  final TextEditingController searchController;
  final double width;
  final bool showDivider;
  final bool isLoading;
  final String? errorMessage;
  final List<ConversationSidebarProject> projects;
  final List<ConversationSidebarConversation> pinnedConversations;
  final List<ConversationSidebarConversation> conversations;
  final String? selectedProjectId;
  final String? selectedConversationId;
  final ValueChanged<String>? onSelectProject;
  final ValueChanged<String>? onSelectConversation;
  final ValueChanged<String>? onToggleConversationPinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;
  final VoidCallback? onCreateProject;
  final bool projectsLoading;
  final String? projectLoadError;
  final ValueChanged<String>? onOpenProjectOptions;
  final ValueChanged<String>? onCreateProjectConversation;
  final ValueChanged<String>? onShowMoreProjectConversations;
  final VoidCallback? onCreateConversation;
  final ValueChanged<String>? onMoveConversationToChats;
  final ValueChanged<String>? onPinConversation;

  @override
  Widget build(BuildContext context) {
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;

    return Container(
      width: width,
      decoration: BoxDecoration(color: palette.surface),
      foregroundDecoration: showDivider
          ? BoxDecoration(
              border: Border(right: BorderSide(color: palette.border)),
            )
          : null,
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 21, 20, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                height: 28,
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        l10n.chats,
                        style: Theme.of(context).textTheme.titleLarge,
                      ),
                    ),
                    IconButton(
                      tooltip: l10n.newConversation,
                      onPressed: onCreateConversation,
                      visualDensity: VisualDensity.compact,
                      constraints: const BoxConstraints.tightFor(
                        width: 28,
                        height: 28,
                      ),
                      padding: EdgeInsets.zero,
                      icon: Icon(
                        Icons.add_rounded,
                        color: palette.secondaryIcon,
                        size: 19,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 12),
              SizedBox(
                height: 36,
                child: TextField(
                  controller: searchController,
                  textInputAction: TextInputAction.search,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 13,
                    fontWeight: FontWeight.w400,
                    height: 18 / 13,
                  ),
                  decoration: InputDecoration(
                    hintText: l10n.searchChatsHint,
                    hintStyle: TextStyle(
                      color: palette.secondaryText,
                      fontSize: 13,
                      fontWeight: FontWeight.w400,
                      height: 18 / 13,
                    ),
                    filled: true,
                    fillColor: palette.hover,
                    prefixIcon: Padding(
                      padding: const EdgeInsets.fromLTRB(10, 10, 8, 10),
                      child: SvgPicture.asset(
                        dark
                            ? 'assets/icons/dark/search.svg'
                            : 'assets/icons/search.svg',
                        width: 16,
                        height: 16,
                        colorFilter: ColorFilter.mode(
                          palette.secondaryText,
                          BlendMode.srcIn,
                        ),
                        excludeFromSemantics: true,
                      ),
                    ),
                    prefixIconConstraints: const BoxConstraints.tightFor(
                      width: 34,
                      height: 36,
                    ),
                    isDense: true,
                    contentPadding: EdgeInsets.zero,
                  ),
                ),
              ),
              const SizedBox(height: 20),
              Divider(height: 1, color: palette.border),
              const SizedBox(height: 17),
              Expanded(
                child: ListenableBuilder(
                  listenable: searchController,
                  builder: (context, _) {
                    final query = searchController.text.trim().toLowerCase();
                    final hasQuery = query.isNotEmpty;
                    if (isLoading || errorMessage != null) {
                      return ListView(
                        padding: const EdgeInsets.only(
                          top: 4,
                          right: 8,
                          bottom: 16,
                        ),
                        children: [
                          _SidebarSection(
                            title: l10n.chats,
                            emptyMessage: errorMessage ?? l10n.historyLoading,
                          ),
                        ],
                      );
                    }
                    final visibleProjects =
                        <
                          ({
                            ConversationSidebarProject project,
                            List<ConversationSidebarConversation> conversations,
                          })
                        >[];
                    for (final project in projects) {
                      final titleMatches =
                          !hasQuery ||
                          project.title.toLowerCase().contains(query);
                      final matchingConversations = titleMatches
                          ? project.conversations
                          : project.conversations
                                .where(
                                  (conversation) => conversation.title
                                      .toLowerCase()
                                      .contains(query),
                                )
                                .toList();
                      if (titleMatches || matchingConversations.isNotEmpty) {
                        visibleProjects.add((
                          project: project,
                          conversations: matchingConversations,
                        ));
                      }
                    }
                    final visiblePinnedConversations = pinnedConversations
                        .where(
                          (conversation) =>
                              !hasQuery ||
                              conversation.title.toLowerCase().contains(query),
                        )
                        .toList();
                    final visibleConversations = conversations
                        .where(
                          (conversation) =>
                              !hasQuery ||
                              conversation.title.toLowerCase().contains(query),
                        )
                        .toList();

                    if (visibleProjects.isNotEmpty ||
                        visiblePinnedConversations.isNotEmpty ||
                        visibleConversations.isNotEmpty) {
                      return ListView(
                        padding: const EdgeInsets.only(
                          top: 4,
                          right: 8,
                          bottom: 16,
                        ),
                        children: [
                          _ProjectSidebarSection(
                            projects: visibleProjects,
                            selectedProjectId: selectedProjectId,
                            selectedConversationId: selectedConversationId,
                            onSelectProject: onSelectProject,
                            onSelectConversation: onSelectConversation,
                            onOpenProjectOptions: onOpenProjectOptions,
                            onCreateProjectConversation:
                                onCreateProjectConversation,
                            onShowMoreProjectConversations:
                                onShowMoreProjectConversations,
                            onMoveConversationToProject:
                                onMoveConversationToProject,
                            onToggleConversationPinned:
                                onToggleConversationPinned,
                            onRenameConversation: onRenameConversation,
                            onDeleteConversation: onDeleteConversation,
                            onExportConversation: onExportConversation,
                            onCreateProject: onCreateProject,
                            loading: projectsLoading,
                            errorMessage: projectLoadError,
                            emptyMessage: hasQuery
                                ? l10n.noChatsSearchTitle
                                : l10n.noProjects,
                          ),
                          const SizedBox(height: 22),
                          _SidebarConversationSection(
                            title: l10n.pinnedChats,
                            emptyMessage: hasQuery
                                ? l10n.noChatsSearchTitle
                                : l10n.noPinnedChats,
                            conversations: visiblePinnedConversations,
                            selectedConversationId: selectedConversationId,
                            itemHeight: 34,
                            dropIcon: Icons.push_pin_outlined,
                            onSelectConversation: onSelectConversation,
                            onTogglePinned: onToggleConversationPinned,
                            onRenameConversation: onRenameConversation,
                            onDeleteConversation: onDeleteConversation,
                            onExportConversation: onExportConversation,
                            onDropConversation: onPinConversation,
                          ),
                          const SizedBox(height: 24),
                          _SidebarConversationSection(
                            title: l10n.chats,
                            emptyMessage: hasQuery
                                ? l10n.noChatsSearchTitle
                                : l10n.noChatsTitle,
                            conversations: visibleConversations,
                            selectedConversationId: selectedConversationId,
                            itemHeight: 32,
                            dropIcon: Icons.chat_bubble_outline_rounded,
                            onSelectConversation: onSelectConversation,
                            onTogglePinned: onToggleConversationPinned,
                            onRenameConversation: onRenameConversation,
                            onDeleteConversation: onDeleteConversation,
                            onExportConversation: onExportConversation,
                            onDropConversation: onMoveConversationToChats,
                          ),
                        ],
                      );
                    }

                    return ListView(
                      padding: const EdgeInsets.only(
                        top: 4,
                        right: 8,
                        bottom: 16,
                      ),
                      children: [
                        _ProjectSidebarSection(
                          projects: const [],
                          selectedProjectId: selectedProjectId,
                          selectedConversationId: selectedConversationId,
                          onSelectProject: onSelectProject,
                          onSelectConversation: onSelectConversation,
                          onOpenProjectOptions: onOpenProjectOptions,
                          onCreateProjectConversation:
                              onCreateProjectConversation,
                          onShowMoreProjectConversations:
                              onShowMoreProjectConversations,
                          onMoveConversationToProject:
                              onMoveConversationToProject,
                          onToggleConversationPinned:
                              onToggleConversationPinned,
                          onRenameConversation: onRenameConversation,
                          onDeleteConversation: onDeleteConversation,
                          onExportConversation: onExportConversation,
                          onCreateProject: onCreateProject,
                          loading: projectsLoading,
                          errorMessage: projectLoadError,
                          emptyMessage: hasQuery
                              ? l10n.noChatsSearchTitle
                              : l10n.noProjects,
                        ),
                        const SizedBox(height: 24),
                        _SidebarSection(
                          title: l10n.pinnedChats,
                          emptyMessage: l10n.noPinnedChats,
                        ),
                        const SizedBox(height: 24),
                        _SidebarSection(
                          title: l10n.chats,
                          emptyMessage: hasQuery
                              ? l10n.noChatsSearchTitle
                              : l10n.noChatsTitle,
                        ),
                      ],
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ProjectSidebarSection extends StatelessWidget {
  const _ProjectSidebarSection({
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
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(child: _SidebarSectionHeading(title: l10n.projects)),
            IconButton(
              tooltip: l10n.createProject,
              onPressed: onCreateProject,
              visualDensity: VisualDensity.compact,
              constraints: const BoxConstraints.tightFor(width: 28, height: 28),
              padding: EdgeInsets.zero,
              icon: const Icon(Icons.add_rounded, size: 18),
            ),
          ],
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
                            _DraggableSidebarConversation(
                              conversation: entry.conversations[index],
                              child: _SidebarConversationTile(
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

class _SidebarSectionHeading extends StatelessWidget {
  const _SidebarSectionHeading({required this.title});

  final String title;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 18,
      child: Text(
        title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: Theme.of(context).textTheme.bodyMedium?.copyWith(
          fontSize: 12,
          fontWeight: FontWeight.w500,
          height: 18 / 12,
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
    final l10n = context.zihoraL10n;
    final palette = ZihoraPalette.of(context);
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

class _SidebarConversationSection extends StatelessWidget {
  const _SidebarConversationSection({
    required this.title,
    required this.emptyMessage,
    required this.conversations,
    required this.selectedConversationId,
    required this.itemHeight,
    required this.dropIcon,
    required this.onSelectConversation,
    required this.onTogglePinned,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onExportConversation,
    required this.onDropConversation,
  });

  final String title;
  final String emptyMessage;
  final List<ConversationSidebarConversation> conversations;
  final String? selectedConversationId;
  final double itemHeight;
  final IconData dropIcon;
  final ValueChanged<String>? onSelectConversation;
  final ValueChanged<String>? onTogglePinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final ValueChanged<String>? onDropConversation;

  @override
  Widget build(BuildContext context) {
    return DragTarget<String>(
      onWillAcceptWithDetails: (_) => onDropConversation != null,
      onAcceptWithDetails: (details) => onDropConversation?.call(details.data),
      builder: (context, candidates, _) {
        final isDropTarget = candidates.isNotEmpty;
        final palette = ZihoraPalette.of(context);
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: isDropTarget ? palette.hover : Colors.transparent,
                borderRadius: BorderRadius.circular(6),
              ),
              child: Row(
                children: [
                  Expanded(child: _SidebarSectionHeading(title: title)),
                  if (isDropTarget)
                    Icon(dropIcon, size: 16, color: palette.accentIcon),
                ],
              ),
            ),
            const SizedBox(height: 6),
            if (conversations.isEmpty)
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 0, 4, 4),
                child: Text(
                  emptyMessage,
                  style: TextStyle(
                    color: palette.secondaryText,
                    fontSize: 12,
                    height: 18 / 12,
                  ),
                ),
              )
            else
              for (var index = 0; index < conversations.length; index++) ...[
                if (index > 0) const SizedBox(height: 4),
                _DraggableSidebarConversation(
                  conversation: conversations[index],
                  child: _SidebarConversationTile(
                    conversation: conversations[index],
                    selected: conversations[index].id == selectedConversationId,
                    height: itemHeight,
                    inset: 10,
                    showChatIcon: true,
                    onPressed: onSelectConversation == null
                        ? null
                        : () => onSelectConversation!(conversations[index].id),
                    onTogglePinned: onTogglePinned == null
                        ? null
                        : () => onTogglePinned!(conversations[index].id),
                    onRename: onRenameConversation == null
                        ? null
                        : () => onRenameConversation!(conversations[index].id),
                    onDelete: onDeleteConversation == null
                        ? null
                        : () => onDeleteConversation!(conversations[index].id),
                    onExport: onExportConversation == null
                        ? null
                        : () => onExportConversation!(conversations[index].id),
                    key: ValueKey<String>(
                      'sidebar-conversation-${conversations[index].id}',
                    ),
                  ),
                ),
              ],
          ],
        );
      },
    );
  }
}

class _DraggableSidebarConversation extends StatelessWidget {
  const _DraggableSidebarConversation({
    required this.conversation,
    required this.child,
  });

  final ConversationSidebarConversation conversation;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);

    return Draggable<String>(
      data: conversation.id,
      maxSimultaneousDrags: 1,
      feedback: Material(
        color: Colors.transparent,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 280),
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
          decoration: BoxDecoration(
            color: palette.composer,
            border: Border.all(color: palette.border),
            borderRadius: BorderRadius.circular(7),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.chat_bubble_outline_rounded,
                size: 15,
                color: palette.secondaryIcon,
              ),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  conversation.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.text, fontSize: 13),
                ),
              ),
            ],
          ),
        ),
      ),
      childWhenDragging: Opacity(opacity: 0.45, child: child),
      child: child,
    );
  }
}

class _SidebarConversationTile extends StatefulWidget {
  const _SidebarConversationTile({
    required this.conversation,
    required this.selected,
    required this.height,
    required this.inset,
    required this.onPressed,
    this.showChatIcon = false,
    this.onTogglePinned,
    this.onRename,
    this.onDelete,
    this.onExport,
    super.key,
  });

  final ConversationSidebarConversation conversation;
  final bool selected;
  final double height;
  final double inset;
  final VoidCallback? onPressed;
  final bool showChatIcon;
  final VoidCallback? onTogglePinned;
  final VoidCallback? onRename;
  final VoidCallback? onDelete;
  final VoidCallback? onExport;

  @override
  State<_SidebarConversationTile> createState() =>
      _SidebarConversationTileState();
}

class _SidebarConversationTileState extends State<_SidebarConversationTile> {
  bool _hovered = false;
  bool _menuOpen = false;

  @override
  Widget build(BuildContext context) {
    final palette = ZihoraPalette.of(context);
    final l10n = context.zihoraL10n;
    final hasActions =
        widget.onTogglePinned != null ||
        widget.onRename != null ||
        widget.onDelete != null ||
        widget.onExport != null;
    final showActions =
        hasActions &&
        (_hovered ||
            widget.selected ||
            _menuOpen ||
            MediaQuery.sizeOf(context).width < ZihoraSpacing.sidebarBreakpoint);

    return MouseRegion(
      onEnter: hasActions ? (_) => setState(() => _hovered = true) : null,
      onExit: hasActions ? (_) => setState(() => _hovered = false) : null,
      child: Semantics(
        button: true,
        enabled: widget.onPressed != null,
        selected: widget.selected,
        label: widget.conversation.title,
        child: Material(
          color: widget.selected ? palette.selected : Colors.transparent,
          borderRadius: BorderRadius.circular(7),
          child: InkWell(
            onTap: widget.onPressed,
            hoverColor: palette.selected,
            borderRadius: BorderRadius.circular(7),
            child: SizedBox(
              height: hasActions ? 40 : widget.height,
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: widget.inset),
                child: Row(
                  children: [
                    if (widget.showChatIcon) ...[
                      Icon(
                        Icons.chat_bubble_outline_rounded,
                        size: 15,
                        color: palette.secondaryIcon,
                      ),
                      const SizedBox(width: 8),
                    ],
                    Expanded(
                      child: Text(
                        widget.conversation.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 13,
                          fontWeight: widget.selected
                              ? FontWeight.w500
                              : FontWeight.w400,
                          height: 18 / 13,
                        ),
                      ),
                    ),
                    if (hasActions) ...[
                      const SizedBox(width: 4),
                      IgnorePointer(
                        ignoring: !showActions,
                        child: AnimatedOpacity(
                          opacity: showActions ? 1 : 0,
                          duration: const Duration(milliseconds: 120),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              if (widget.onTogglePinned != null)
                                IconButton(
                                  tooltip: widget.conversation.isPinned
                                      ? l10n.unpinConversation
                                      : l10n.pinConversation,
                                  visualDensity: VisualDensity.compact,
                                  padding: EdgeInsets.zero,
                                  constraints: const BoxConstraints.tightFor(
                                    width: 32,
                                    height: 36,
                                  ),
                                  onPressed: widget.onTogglePinned,
                                  icon: Icon(
                                    widget.conversation.isPinned
                                        ? Icons.push_pin_rounded
                                        : Icons.push_pin_outlined,
                                    size: 15,
                                  ),
                                ),
                              if (widget.onRename != null ||
                                  widget.onDelete != null ||
                                  widget.onExport != null)
                                SizedBox(
                                  width: 32,
                                  height: 36,
                                  child: PopupMenuButton<String>(
                                    tooltip: l10n.moreOptions,
                                    position: PopupMenuPosition.under,
                                    padding: EdgeInsets.zero,
                                    iconSize: 17,
                                    onOpened: () =>
                                        setState(() => _menuOpen = true),
                                    onCanceled: () =>
                                        setState(() => _menuOpen = false),
                                    onSelected: (value) {
                                      setState(() => _menuOpen = false);
                                      if (value == 'rename') {
                                        widget.onRename?.call();
                                      } else if (value == 'export') {
                                        widget.onExport?.call();
                                      } else if (value == 'delete') {
                                        widget.onDelete?.call();
                                      }
                                    },
                                    itemBuilder: (context) => [
                                      if (widget.onRename != null)
                                        PopupMenuItem<String>(
                                          value: 'rename',
                                          child: Text(l10n.renameConversation),
                                        ),
                                      if (widget.onExport != null)
                                        PopupMenuItem<String>(
                                          value: 'export',
                                          child: Text(l10n.exportConversation),
                                        ),
                                      if (widget.onDelete != null)
                                        PopupMenuItem<String>(
                                          value: 'delete',
                                          child: Text(
                                            l10n.deleteConversation,
                                            style: TextStyle(
                                              color: Theme.of(
                                                context,
                                              ).colorScheme.error,
                                            ),
                                          ),
                                        ),
                                    ],
                                    icon: const Icon(Icons.more_horiz_rounded),
                                  ),
                                ),
                            ],
                          ),
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
  }
}

class _SidebarSection extends StatelessWidget {
  const _SidebarSection({required this.title, required this.emptyMessage});

  final String title;
  final String emptyMessage;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return SizedBox(
      height: 56,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            height: 18,
            child: Text(
              title,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: textTheme.bodyMedium?.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w500,
                height: 18 / 13,
              ),
            ),
          ),
          const SizedBox(height: 8),
          SizedBox(
            height: 18,
            child: Text(
              emptyMessage,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: textTheme.bodyMedium?.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w400,
                height: 18 / 13,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
