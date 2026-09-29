import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/project_sidebar_section.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_section.dart';

class ConversationSidebar extends StatelessWidget {
  const ConversationSidebar({
    required this.searchController,
    this.width = OpenChatSpacing.sidebarWidth,
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
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final dark = Theme.of(context).brightness == Brightness.dark;

    return Container(
      width: width,
      decoration: BoxDecoration(color: palette.navigation),
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
                          ProjectSidebarSection(
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
                          SidebarConversationSection(
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
                          SidebarConversationSection(
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
                        ProjectSidebarSection(
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
