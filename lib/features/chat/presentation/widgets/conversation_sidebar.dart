import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/presentation/widgets/project_sidebar_section.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_collapsible_heading.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_section.dart';

class ConversationSidebar extends StatelessWidget {
  const ConversationSidebar({
    required this.searchController,
    this.isHistorySearchOpen = false,
    this.onOpenHistorySearch,
    this.onOpenRunManager,
    this.onCloseHistorySearch,
    this.onSearchChanged,
    this.onSearchSubmitted,
    this.historySearchQuery,
    this.historySearchResults = const <HistorySearchResult>[],
    this.isHistorySearchLoading = false,
    this.historySearchErrorCode,
    this.onSelectHistoryResult,
    this.width = OpenChatSpacing.sidebarWidth,
    this.showDivider = true,
    this.isLoading = false,
    this.errorMessage,
    this.onRetryStorage,
    this.projects = const <ConversationSidebarProject>[],
    this.pinnedConversations = const <ConversationSidebarConversation>[],
    this.conversations = const <ConversationSidebarConversation>[],
    this.archivedConversations = const <ConversationSidebarConversation>[],
    this.selectedProjectId,
    this.selectedConversationId,
    this.onSelectProject,
    this.onSelectConversation,
    this.onToggleConversationPinned,
    this.onRenameConversation,
    this.onDeleteConversation,
    this.onExportConversation,
    this.onArchiveConversation,
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
    this.collapsedSections = const <String>{},
    this.onToggleSection,
    super.key,
  });

  final TextEditingController searchController;
  final bool isHistorySearchOpen;
  final VoidCallback? onOpenHistorySearch;
  final VoidCallback? onOpenRunManager;
  final VoidCallback? onCloseHistorySearch;
  final ValueChanged<String>? onSearchChanged;
  final ValueChanged<String>? onSearchSubmitted;
  final String? historySearchQuery;
  final List<HistorySearchResult> historySearchResults;
  final bool isHistorySearchLoading;
  final String? historySearchErrorCode;
  final ValueChanged<HistorySearchResult>? onSelectHistoryResult;
  final double width;
  final bool showDivider;
  final bool isLoading;
  final String? errorMessage;
  final VoidCallback? onRetryStorage;
  final List<ConversationSidebarProject> projects;
  final List<ConversationSidebarConversation> pinnedConversations;
  final List<ConversationSidebarConversation> conversations;
  final List<ConversationSidebarConversation> archivedConversations;
  final String? selectedProjectId;
  final String? selectedConversationId;
  final ValueChanged<String>? onSelectProject;
  final ValueChanged<String>? onSelectConversation;
  final ValueChanged<String>? onToggleConversationPinned;
  final ValueChanged<String>? onRenameConversation;
  final ValueChanged<String>? onDeleteConversation;
  final ValueChanged<String>? onExportConversation;
  final ValueChanged<String>? onArchiveConversation;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;
  final VoidCallback? onCreateProject;
  final bool projectsLoading;
  final String? projectLoadError;
  final void Function(String projectId, String projectRoot, String projectName)?
  onOpenProjectOptions;
  final ValueChanged<String>? onCreateProjectConversation;
  final ValueChanged<String>? onShowMoreProjectConversations;
  final VoidCallback? onCreateConversation;
  final ValueChanged<String>? onMoveConversationToChats;
  final ValueChanged<String>? onPinConversation;
  final Set<String> collapsedSections;
  final ValueChanged<String>? onToggleSection;

  bool _isCollapsed(String section) => collapsedSections.contains(section);

  VoidCallback _toggleSection(String section) =>
      () => onToggleSection?.call(section);

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
              border: Border(
                right: BorderSide(
                  color: palette.border.withValues(alpha: 0.48),
                ),
              ),
            )
          : null,
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ConstrainedBox(
                key: const ValueKey<String>('conversation-sidebar-header'),
                constraints: const BoxConstraints(
                  minHeight: OpenChatSpacing.conversationHeaderHeight,
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                  child: Row(
                    children: [
                      Expanded(
                        child: AnimatedSwitcher(
                          duration: MediaQuery.disableAnimationsOf(context)
                              ? Duration.zero
                              : const Duration(milliseconds: 180),
                          transitionBuilder: (child, animation) => ClipRect(
                            child: SizeTransition(
                              axis: Axis.horizontal,
                              alignment: Alignment.centerRight,
                              sizeFactor: animation,
                              child: FadeTransition(
                                opacity: animation,
                                child: child,
                              ),
                            ),
                          ),
                          child: isHistorySearchOpen
                              ? TextField(
                                  key: const ValueKey<String>(
                                    'history-search-input',
                                  ),
                                  autofocus: true,
                                  controller: searchController,
                                  onChanged: onSearchChanged,
                                  onSubmitted: onSearchSubmitted,
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
                                      padding: const EdgeInsets.fromLTRB(
                                        10,
                                        10,
                                        8,
                                        10,
                                      ),
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
                                    prefixIconConstraints:
                                        const BoxConstraints.tightFor(
                                          width: 34,
                                          height: 36,
                                        ),
                                    suffixIcon: isHistorySearchLoading
                                        ? const SizedBox(
                                            width: 34,
                                            height: 36,
                                            child: Center(
                                              child: SizedBox.square(
                                                dimension: 15,
                                                child:
                                                    CircularProgressIndicator(
                                                      strokeWidth: 2,
                                                    ),
                                              ),
                                            ),
                                          )
                                        : null,
                                    isDense: true,
                                    contentPadding: EdgeInsets.zero,
                                  ),
                                )
                              : Align(
                                  key: const ValueKey<String>(
                                    'conversation-sidebar-title',
                                  ),
                                  alignment: Alignment.centerLeft,
                                  child: Text(
                                    'OpenChat',
                                    style: TextStyle(
                                      color: palette.text,
                                      fontSize: 16,
                                      fontWeight: FontWeight.w400,
                                      height: 22 / 16,
                                    ),
                                  ),
                                ),
                        ),
                      ),
                      if (onOpenRunManager != null)
                        IconButton(
                          tooltip: l10n.agentRunManagerTitle,
                          onPressed: onOpenRunManager,
                          visualDensity: VisualDensity.compact,
                          constraints: const BoxConstraints.tightFor(
                            width: 40,
                            height: 40,
                          ),
                          padding: EdgeInsets.zero,
                          icon: Icon(
                            LucideIcons.activity,
                            color: palette.secondaryIcon,
                            size: 17,
                          ),
                        ),
                      IconButton(
                        tooltip: isHistorySearchOpen
                            ? l10n.close
                            : l10n.searchMessagesTooltip,
                        onPressed: isHistorySearchOpen
                            ? onCloseHistorySearch
                            : onOpenHistorySearch,
                        visualDensity: VisualDensity.compact,
                        constraints: const BoxConstraints.tightFor(
                          width: 40,
                          height: 40,
                        ),
                        padding: EdgeInsets.zero,
                        icon: isHistorySearchOpen
                            ? Icon(
                                LucideIcons.x,
                                color: palette.secondaryIcon,
                                size: 18,
                              )
                            : SvgPicture.asset(
                                dark
                                    ? 'assets/icons/dark/search.svg'
                                    : 'assets/icons/search.svg',
                                width: 17,
                                height: 17,
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
              Tooltip(
                message: l10n.newChat,
                child: TextButton.icon(
                  onPressed: onCreateConversation,
                  icon: Icon(
                    LucideIcons.messageSquarePlus,
                    color: palette.secondaryIcon,
                    size: 18,
                  ),
                  label: Text(
                    l10n.newChat,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 13,
                      fontWeight: FontWeight.w400,
                      height: 18 / 13,
                    ),
                  ),
                  style: TextButton.styleFrom(
                    alignment: Alignment.centerLeft,
                    minimumSize: const Size.fromHeight(44),
                    padding: const EdgeInsets.fromLTRB(14, 0, 8, 0),
                    tapTargetSize: MaterialTapTargetSize.padded,
                    visualDensity: VisualDensity.compact,
                    foregroundColor: palette.text,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 8),
              Expanded(
                child: Stack(
                  children: [
                    Positioned.fill(
                      child: ListenableBuilder(
                        listenable: searchController,
                        builder: (context, _) {
                          final query = searchController.text
                              .trim()
                              .toLowerCase();
                          final hasQuery = query.isNotEmpty;
                          if (isLoading || errorMessage != null) {
                            return ListView(
                              padding: const EdgeInsets.only(
                                top: 4,
                                left: 4,
                                right: 0,
                                bottom: 12,
                              ),
                              children: [
                                _SidebarSection(
                                  title: l10n.chats,
                                  emptyMessage:
                                      errorMessage ?? l10n.historyLoading,
                                  onRetry: onRetryStorage,
                                ),
                              ],
                            );
                          }
                          final visibleProjects =
                              <
                                ({
                                  ConversationSidebarProject project,
                                  List<ConversationSidebarConversation>
                                  conversations,
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
                            if (titleMatches ||
                                matchingConversations.isNotEmpty) {
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
                                    conversation.title.toLowerCase().contains(
                                      query,
                                    ),
                              )
                              .toList();
                          final visibleConversations = conversations
                              .where(
                                (conversation) =>
                                    !hasQuery ||
                                    conversation.title.toLowerCase().contains(
                                      query,
                                    ),
                              )
                              .toList();
                          final visibleArchivedConversations =
                              archivedConversations
                                  .where(
                                    (conversation) =>
                                        !hasQuery ||
                                        conversation.title
                                            .toLowerCase()
                                            .contains(query),
                                  )
                                  .toList();

                          final hasTitleResults =
                              visibleProjects.isNotEmpty ||
                              visiblePinnedConversations.isNotEmpty ||
                              visibleConversations.isNotEmpty ||
                              visibleArchivedConversations.isNotEmpty;
                          if (hasTitleResults) {
                            return ListView(
                              padding: const EdgeInsets.only(
                                top: 4,
                                left: 4,
                                right: 0,
                                bottom: 12,
                              ),
                              children: [
                                SidebarConversationSection(
                                  title: l10n.pinnedChats,
                                  emptyMessage: hasQuery
                                      ? l10n.noChatsSearchTitle
                                      : l10n.noPinnedChats,
                                  conversations: visiblePinnedConversations,
                                  selectedConversationId:
                                      selectedConversationId,
                                  itemHeight: 32,
                                  dropIcon: LucideIcons.pin,
                                  onSelectConversation: onSelectConversation,
                                  onTogglePinned: onToggleConversationPinned,
                                  onRenameConversation: onRenameConversation,
                                  onDeleteConversation: onDeleteConversation,
                                  onExportConversation: onExportConversation,
                                  onArchiveConversation: onArchiveConversation,
                                  onDropConversation: onPinConversation,
                                  collapsed: _isCollapsed('pinned'),
                                  onToggleCollapsed: _toggleSection('pinned'),
                                ),
                                const SizedBox(height: 16),
                                ProjectSidebarSection(
                                  projects: visibleProjects,
                                  collapsed: _isCollapsed('projects'),
                                  onToggleCollapsed: _toggleSection('projects'),
                                  selectedProjectId: selectedProjectId,
                                  selectedConversationId:
                                      selectedConversationId,
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
                                  onArchiveConversation: onArchiveConversation,
                                  onCreateProject: onCreateProject,
                                  loading: projectsLoading,
                                  errorMessage: projectLoadError,
                                  emptyMessage: hasQuery
                                      ? l10n.noChatsSearchTitle
                                      : l10n.noProjects,
                                ),
                                const SizedBox(height: 16),
                                SidebarConversationSection(
                                  title: l10n.chats,
                                  emptyMessage: hasQuery
                                      ? l10n.noChatsSearchTitle
                                      : l10n.noChatsTitle,
                                  conversations: visibleConversations,
                                  selectedConversationId:
                                      selectedConversationId,
                                  itemHeight: 32,
                                  dropIcon: LucideIcons.messageCircle,
                                  onSelectConversation: onSelectConversation,
                                  onTogglePinned: onToggleConversationPinned,
                                  onRenameConversation: onRenameConversation,
                                  onDeleteConversation: onDeleteConversation,
                                  onExportConversation: onExportConversation,
                                  onArchiveConversation: onArchiveConversation,
                                  onDropConversation: onMoveConversationToChats,
                                  collapsed: _isCollapsed('chats'),
                                  onToggleCollapsed: _toggleSection('chats'),
                                ),
                                const SizedBox(height: 16),
                                SidebarConversationSection(
                                  title: l10n.archivedChats,
                                  emptyMessage: hasQuery
                                      ? l10n.noChatsSearchTitle
                                      : l10n.noArchivedChats,
                                  conversations: visibleArchivedConversations,
                                  selectedConversationId:
                                      selectedConversationId,
                                  itemHeight: 32,
                                  dropIcon: LucideIcons.archive,
                                  onSelectConversation: onSelectConversation,
                                  onTogglePinned: null,
                                  onRenameConversation: onRenameConversation,
                                  onDeleteConversation: onDeleteConversation,
                                  onExportConversation: onExportConversation,
                                  onDropConversation: null,
                                  isArchivedSection: true,
                                  onArchiveConversation: onArchiveConversation,
                                  collapsed: _isCollapsed('archived'),
                                  onToggleCollapsed: _toggleSection('archived'),
                                ),
                              ],
                            );
                          }

                          return ListView(
                            padding: const EdgeInsets.only(
                              top: 4,
                              left: 4,
                              right: 0,
                              bottom: 12,
                            ),
                            children: [
                              _SidebarSection(
                                title: l10n.pinnedChats,
                                emptyMessage: l10n.noPinnedChats,
                                collapsed: _isCollapsed('pinned'),
                                onToggleCollapsed: _toggleSection('pinned'),
                              ),
                              const SizedBox(height: 16),
                              ProjectSidebarSection(
                                projects: const [],
                                collapsed: _isCollapsed('projects'),
                                onToggleCollapsed: _toggleSection('projects'),
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
                              const SizedBox(height: 16),
                              _SidebarSection(
                                title: l10n.chats,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noChatsTitle,
                                collapsed: _isCollapsed('chats'),
                                onToggleCollapsed: _toggleSection('chats'),
                              ),
                              const SizedBox(height: 16),
                              SidebarConversationSection(
                                title: l10n.archivedChats,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noArchivedChats,
                                conversations: visibleArchivedConversations,
                                selectedConversationId: selectedConversationId,
                                itemHeight: 32,
                                dropIcon: LucideIcons.archive,
                                onSelectConversation: onSelectConversation,
                                onTogglePinned: null,
                                onRenameConversation: onRenameConversation,
                                onDeleteConversation: onDeleteConversation,
                                onExportConversation: onExportConversation,
                                onDropConversation: null,
                                isArchivedSection: true,
                                onArchiveConversation: onArchiveConversation,
                                collapsed: _isCollapsed('archived'),
                                onToggleCollapsed: _toggleSection('archived'),
                              ),
                            ],
                          );
                        },
                      ),
                    ),
                    if (isHistorySearchOpen &&
                        searchController.text.trim().isNotEmpty &&
                        historySearchQuery?.trim().toLowerCase() ==
                            searchController.text.trim().toLowerCase())
                      Positioned(
                        top: 0,
                        left: 0,
                        right: 0,
                        child: Material(
                          color: palette.navigation,
                          elevation: 5,
                          clipBehavior: Clip.antiAlias,
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(8),
                            side: BorderSide(color: palette.border),
                          ),
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(maxHeight: 360),
                            child: SingleChildScrollView(
                              padding: const EdgeInsets.fromLTRB(8, 10, 8, 8),
                              child: _HistorySearchResultsSection(
                                query: searchController.text.trim(),
                                results: historySearchResults,
                                loading: isHistorySearchLoading,
                                errorCode: historySearchErrorCode,
                                onSearch: onSearchSubmitted,
                                onSelectResult: onSelectHistoryResult,
                              ),
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _HistorySearchResultsSection extends StatelessWidget {
  const _HistorySearchResultsSection({
    required this.query,
    required this.results,
    required this.loading,
    required this.errorCode,
    required this.onSearch,
    required this.onSelectResult,
  });

  final String query;
  final List<HistorySearchResult> results;
  final bool loading;
  final String? errorCode;
  final ValueChanged<String>? onSearch;
  final ValueChanged<HistorySearchResult>? onSelectResult;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final message = switch (errorCode) {
      'too_short' => l10n.searchMessagesTooShort,
      'too_long' => l10n.searchMessagesTooLong,
      'failed' => l10n.searchMessagesFailed,
      _ => null,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(4, 0, 4, 7),
          child: Text(
            l10n.searchMessagesHeader,
            style: theme.textTheme.bodyMedium?.copyWith(
              color: palette.secondaryText,
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
        if (loading)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
            child: Semantics(
              label: l10n.searchMessagesLoading,
              child: const LinearProgressIndicator(minHeight: 2),
            ),
          )
        else if (message != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 4, 4, 8),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    message,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: palette.secondaryText,
                    ),
                  ),
                ),
                if (errorCode == 'failed' && onSearch != null)
                  IconButton(
                    tooltip: l10n.retry,
                    onPressed: () => onSearch!(query),
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(LucideIcons.refreshCw, size: 17),
                  ),
              ],
            ),
          )
        else if (results.isEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 4, 4, 8),
            child: Text(
              l10n.searchMessagesNoResults,
              style: theme.textTheme.bodySmall?.copyWith(
                color: palette.secondaryText,
              ),
            ),
          )
        else
          for (final result in results)
            Material(
              color: Colors.transparent,
              child: InkWell(
                onTap: onSelectResult == null
                    ? null
                    : () => onSelectResult!(result),
                borderRadius: BorderRadius.circular(6),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 5,
                    vertical: 7,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Text(
                        result.conversationTitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: palette.text,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      const SizedBox(height: 3),
                      Text.rich(
                        TextSpan(
                          children: _historyExcerptSpans(
                            result.excerpt,
                            normalStyle: theme.textTheme.bodySmall?.copyWith(
                              color: palette.secondaryText,
                            ),
                            matchStyle: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onPrimaryContainer,
                              backgroundColor:
                                  theme.colorScheme.primaryContainer,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ),
            ),
      ],
    );
  }
}

List<InlineSpan> _historyExcerptSpans(
  String excerpt, {
  required TextStyle? normalStyle,
  required TextStyle? matchStyle,
}) {
  const opening = '[match]';
  const closing = '[/match]';
  final spans = <InlineSpan>[];
  var offset = 0;
  while (offset < excerpt.length) {
    final openingIndex = excerpt.indexOf(opening, offset);
    if (openingIndex < 0) {
      spans.add(TextSpan(text: excerpt.substring(offset), style: normalStyle));
      break;
    }
    if (openingIndex > offset) {
      spans.add(
        TextSpan(
          text: excerpt.substring(offset, openingIndex),
          style: normalStyle,
        ),
      );
    }
    final matchStart = openingIndex + opening.length;
    final closingIndex = excerpt.indexOf(closing, matchStart);
    if (closingIndex < 0) {
      spans.add(
        TextSpan(text: excerpt.substring(matchStart), style: matchStyle),
      );
      break;
    }
    spans.add(
      TextSpan(
        text: excerpt.substring(matchStart, closingIndex),
        style: matchStyle,
      ),
    );
    offset = closingIndex + closing.length;
  }
  return spans;
}

class _SidebarSection extends StatelessWidget {
  const _SidebarSection({
    required this.title,
    required this.emptyMessage,
    this.onRetry,
    this.collapsed = false,
    this.onToggleCollapsed,
  });

  final String title;
  final String emptyMessage;
  final VoidCallback? onRetry;
  final bool collapsed;
  final VoidCallback? onToggleCollapsed;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final palette = OpenChatPalette.of(context);

    return ConstrainedBox(
      constraints: BoxConstraints(
        minHeight: collapsed
            ? 28
            : onRetry == null
            ? 56
            : 104,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SidebarCollapsibleHeading(
            title: title,
            collapsed: collapsed,
            onPressed: onToggleCollapsed ?? () {},
          ),
          if (!collapsed) const SizedBox(height: 8),
          if (!collapsed)
            Padding(
              padding: EdgeInsets.only(bottom: onRetry == null ? 0 : 2),
              child: Text(
                emptyMessage,
                maxLines: onRetry == null ? 1 : 2,
                overflow: TextOverflow.ellipsis,
                style: textTheme.bodyMedium?.copyWith(
                  color: palette.secondaryText,
                  fontSize: 12,
                  fontWeight: FontWeight.w400,
                  height: 18 / 12,
                ),
              ),
            ),
          if (onRetry case final retry?)
            TextButton.icon(
              onPressed: retry,
              icon: const Icon(LucideIcons.refreshCw, size: 16),
              label: Text(context.openchatL10n.retry),
              style: TextButton.styleFrom(
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 8),
              ),
            ),
        ],
      ),
    );
  }
}
