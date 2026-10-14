import 'dart:async';

import 'package:flutter/material.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:shadcn_ui/shadcn_ui.dart' as shad;

import 'package:openchat/app/openchat_select.dart';
import 'package:openchat/app/openchat_glass_surface.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/domain/local_library_search_result.dart';
import 'package:openchat/features/chat/presentation/widgets/project_sidebar_section.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_section_heading.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_section.dart';

class ConversationSidebar extends StatelessWidget {
  const ConversationSidebar({
    required this.searchController,
    this.historySearchDialogRevision,
    this.isHistorySearchOpen = false,
    this.onOpenHistorySearch,
    this.onOpenSavedHistorySearches,
    this.onCloseHistorySearch,
    this.onSearchChanged,
    this.onSearchSubmitted,
    this.hasHistorySearchDateRange = false,
    this.onPickHistorySearchDateRange,
    this.onClearHistorySearchDateRange,
    this.historySearchFilterOptions,
    this.isLoadingHistorySearchFilterOptions = false,
    this.historySearchFilters = const HistorySearchFilters(),
    this.onApplyHistorySearchFilters,
    this.historySearchResults = const <HistorySearchResult>[],
    this.localLibrarySearchResults = const <LocalLibrarySearchResult>[],
    this.unifiedLibrarySearchFilter = UnifiedLibrarySearchFilter.all,
    this.isHistorySearchLoading = false,
    this.historySearchErrorCode,
    this.onSelectHistoryResult,
    this.onSelectLocalLibraryResult,
    this.onUnifiedLibrarySearchFilterChanged,
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
    this.onEditConversationTags,
    this.onToggleConversationBookmark,
    this.selectionMode = false,
    this.selectedConversationIds = const <String>{},
    this.onToggleBatchSelection,
    this.onToggleSelectionMode,
    this.onArchiveSelected,
    this.onMoveSelected,
    this.onMoveConversationToProject,
    this.onCreateProject,
    this.projectsLoading = false,
    this.projectLoadError,
    this.onRetryProjects,
    this.onOpenProjectOptions,
    this.onCreateProjectConversation,
    this.onShowMoreProjectConversations,
    this.onCreateConversation,
    this.onCreateTemporaryConversation,
    this.onMoveConversationToChats,
    this.onPinConversation,
    this.collapsedSections = const <String>{},
    this.onToggleSection,
    super.key,
  });

  final TextEditingController searchController;
  final Listenable? historySearchDialogRevision;
  final bool isHistorySearchOpen;
  final VoidCallback? onOpenHistorySearch;
  final VoidCallback? onOpenSavedHistorySearches;
  final VoidCallback? onCloseHistorySearch;
  final ValueChanged<String>? onSearchChanged;
  final ValueChanged<String>? onSearchSubmitted;
  final bool hasHistorySearchDateRange;
  final VoidCallback? onPickHistorySearchDateRange;
  final VoidCallback? onClearHistorySearchDateRange;
  final HistorySearchFilterOptions? historySearchFilterOptions;
  final bool isLoadingHistorySearchFilterOptions;
  final HistorySearchFilters historySearchFilters;
  final ValueChanged<HistorySearchFilters>? onApplyHistorySearchFilters;
  final List<HistorySearchResult> historySearchResults;
  final List<LocalLibrarySearchResult> localLibrarySearchResults;
  final UnifiedLibrarySearchFilter unifiedLibrarySearchFilter;
  final bool isHistorySearchLoading;
  final String? historySearchErrorCode;
  final ValueChanged<HistorySearchResult>? onSelectHistoryResult;
  final ValueChanged<LocalLibrarySearchResult>? onSelectLocalLibraryResult;
  final ValueChanged<UnifiedLibrarySearchFilter>?
  onUnifiedLibrarySearchFilterChanged;
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
  final ValueChanged<String>? onEditConversationTags;
  final ValueChanged<String>? onToggleConversationBookmark;
  final bool selectionMode;
  final Set<String> selectedConversationIds;
  final ValueChanged<String>? onToggleBatchSelection;
  final VoidCallback? onToggleSelectionMode;
  final VoidCallback? onArchiveSelected;
  final ValueChanged<String?>? onMoveSelected;
  final void Function(String conversationId, String projectId)?
  onMoveConversationToProject;
  final VoidCallback? onCreateProject;
  final bool projectsLoading;
  final String? projectLoadError;
  final VoidCallback? onRetryProjects;
  final void Function(String projectId, String projectRoot, String projectName)?
  onOpenProjectOptions;
  final ValueChanged<String>? onCreateProjectConversation;
  final ValueChanged<String>? onShowMoreProjectConversations;
  final VoidCallback? onCreateConversation;
  final VoidCallback? onCreateTemporaryConversation;
  final ValueChanged<String>? onMoveConversationToChats;
  final ValueChanged<String>? onPinConversation;
  final Set<String> collapsedSections;
  final ValueChanged<String>? onToggleSection;

  bool _isCollapsed(String section) => collapsedSections.contains(section);

  VoidCallback _toggleSection(String section) =>
      () => onToggleSection?.call(section);

  Widget _buildBatchSelectionBar(BuildContext context) {
    final l10n = context.openchatL10n;
    if (!selectionMode) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.fromLTRB(4, 0, 8, 8),
      child: Row(
        children: [
          IconButton(
            tooltip: l10n.cancelSelection,
            onPressed: onToggleSelectionMode,
            icon: const Icon(LucideIcons.x),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              l10n.conversationsSelected(selectedConversationIds.length),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          PopupMenuButton<String>(
            tooltip: l10n.moveSelectedChats,
            enabled:
                selectedConversationIds.isNotEmpty && onMoveSelected != null,
            onSelected: (projectId) =>
                onMoveSelected!(projectId.isEmpty ? null : projectId),
            itemBuilder: (context) => [
              PopupMenuItem<String>(value: '', child: Text(l10n.moveToChats)),
              for (final project in projects)
                PopupMenuItem<String>(
                  value: project.id,
                  child: Text(project.title),
                ),
            ],
            icon: const Icon(LucideIcons.folder),
          ),
          IconButton(
            tooltip: l10n.archiveSelectedConversations,
            onPressed: selectedConversationIds.isEmpty
                ? null
                : onArchiveSelected,
            icon: const Icon(LucideIcons.archive),
          ),
        ],
      ),
    );
  }

  Future<void> _showHistorySearchFilters(BuildContext context) async {
    final l10n = context.openchatL10n;
    final options = historySearchFilterOptions;
    String? providerId =
        options?.providerIds.contains(historySearchFilters.providerId) ?? false
        ? historySearchFilters.providerId
        : null;
    String? modelId =
        options?.modelIds.contains(historySearchFilters.modelId) ?? false
        ? historySearchFilters.modelId
        : null;
    String? projectId =
        projects.any((project) => project.id == historySearchFilters.projectId)
        ? historySearchFilters.projectId
        : null;
    String? tag = options?.tags.contains(historySearchFilters.tag) ?? false
        ? historySearchFilters.tag
        : null;
    var resultType = historySearchFilters.resultType;
    var sortOrder = historySearchFilters.sortOrder;
    var archiveState = switch (historySearchFilters.isArchived) {
      true => 'archived',
      false => 'active',
      null => 'all',
    };
    await showModalBottomSheet<void>(
      context: context,
      sheetAnimationStyle: MediaQuery.disableAnimationsOf(context)
          ? AnimationStyle.noAnimation
          : null,
      isScrollControlled: true,
      builder: (sheetContext) => StatefulBuilder(
        builder: (context, setSheetState) {
          final palette = OpenChatPalette.of(context);
          final reducedMotion = MediaQuery.disableAnimationsOf(context);
          return SafeArea(
            child: Padding(
              padding: EdgeInsets.fromLTRB(
                20,
                20,
                20,
                20 + MediaQuery.viewInsetsOf(context).bottom,
              ),
              child: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      l10n.historySearchFiltersTitle,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      l10n.historySearchRouteFilterNote,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 16),
                    OpenChatSelectField<String>(
                      label: l10n.historySearchResultTypeFilter,
                      options: [
                        OpenChatSelectOption<String>(
                          value: 'all',
                          label: l10n.historySearchAllResultTypes,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'messages',
                          label: l10n.historySearchMessagesOnly,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'conversations',
                          label: l10n.historySearchConversationsOnly,
                        ),
                      ],
                      value: resultType,
                      palette: palette,
                      onChanged: (value) =>
                          setSheetState(() => resultType = value),
                    ),
                    const SizedBox(height: 12),
                    OpenChatSelectField<String>(
                      label: l10n.historySearchSortFilter,
                      options: [
                        OpenChatSelectOption<String>(
                          value: 'relevance',
                          label: l10n.historySearchSortRelevance,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'newest',
                          label: l10n.historySearchSortNewest,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'oldest',
                          label: l10n.historySearchSortOldest,
                        ),
                      ],
                      value: sortOrder,
                      palette: palette,
                      onChanged: (value) =>
                          setSheetState(() => sortOrder = value),
                    ),
                    if (isLoadingHistorySearchFilterOptions) ...[
                      const SizedBox(height: 12),
                      Semantics(
                        liveRegion: true,
                        label: l10n.historySearchFiltersLoading,
                        child: ExcludeSemantics(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              if (reducedMotion)
                                const Align(
                                  alignment: AlignmentDirectional.centerStart,
                                  child: Icon(
                                    LucideIcons.loaderCircle,
                                    size: 16,
                                  ),
                                )
                              else
                                const LinearProgressIndicator(),
                              const SizedBox(height: 6),
                              Text(l10n.historySearchFiltersLoading),
                            ],
                          ),
                        ),
                      ),
                    ],
                    const SizedBox(height: 16),
                    OpenChatSelectField<String?>(
                      label: l10n.historySearchProviderFilter,
                      options: [
                        OpenChatSelectOption<String?>(
                          value: null,
                          label: l10n.historySearchAllProviders,
                        ),
                        for (final id
                            in options?.providerIds ?? const <String>[])
                          OpenChatSelectOption<String?>(value: id, label: id),
                      ],
                      value: providerId,
                      palette: palette,
                      onChanged: options == null
                          ? null
                          : (value) => setSheetState(() => providerId = value),
                    ),
                    const SizedBox(height: 12),
                    OpenChatSelectField<String?>(
                      label: l10n.historySearchModelFilter,
                      options: [
                        OpenChatSelectOption<String?>(
                          value: null,
                          label: l10n.historySearchAllModels,
                        ),
                        for (final id in options?.modelIds ?? const <String>[])
                          OpenChatSelectOption<String?>(value: id, label: id),
                      ],
                      value: modelId,
                      palette: palette,
                      onChanged: options == null
                          ? null
                          : (value) => setSheetState(() => modelId = value),
                    ),
                    const SizedBox(height: 12),
                    OpenChatSelectField<String?>(
                      label: l10n.historySearchProjectFilter,
                      options: [
                        OpenChatSelectOption<String?>(
                          value: null,
                          label: l10n.historySearchAllProjects,
                        ),
                        for (final project in projects)
                          OpenChatSelectOption<String?>(
                            value: project.id,
                            label: project.title,
                          ),
                      ],
                      value: projectId,
                      palette: palette,
                      onChanged: (value) =>
                          setSheetState(() => projectId = value),
                    ),
                    const SizedBox(height: 12),
                    OpenChatSelectField<String?>(
                      label: l10n.historySearchTagFilter,
                      options: [
                        OpenChatSelectOption<String?>(
                          value: null,
                          label: l10n.historySearchAllTags,
                        ),
                        for (final value in options?.tags ?? const <String>[])
                          OpenChatSelectOption<String?>(
                            value: value,
                            label: value,
                          ),
                      ],
                      value: tag,
                      palette: palette,
                      onChanged: (value) => setSheetState(() => tag = value),
                    ),
                    const SizedBox(height: 12),
                    OpenChatSelectField<String>(
                      label: l10n.historySearchArchiveFilter,
                      options: [
                        OpenChatSelectOption<String>(
                          value: 'all',
                          label: l10n.historySearchAllStatuses,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'active',
                          label: l10n.historySearchActiveConversations,
                        ),
                        OpenChatSelectOption<String>(
                          value: 'archived',
                          label: l10n.historySearchArchivedConversations,
                        ),
                      ],
                      value: archiveState,
                      palette: palette,
                      onChanged: (value) =>
                          setSheetState(() => archiveState = value),
                    ),
                    const SizedBox(height: 20),
                    Row(
                      children: [
                        TextButton(
                          onPressed: () {
                            onApplyHistorySearchFilters?.call(
                              const HistorySearchFilters(),
                            );
                            Navigator.of(sheetContext).pop();
                          },
                          child: Text(l10n.historySearchClearFilters),
                        ),
                        const Spacer(),
                        FilledButton(
                          onPressed: () {
                            onApplyHistorySearchFilters?.call(
                              HistorySearchFilters(
                                providerId: providerId,
                                modelId: modelId,
                                projectId: projectId,
                                tag: tag,
                                isArchived: switch (archiveState) {
                                  'active' => false,
                                  'archived' => true,
                                  _ => null,
                                },
                                resultType: resultType,
                                sortOrder: sortOrder,
                              ),
                            );
                            Navigator.of(sheetContext).pop();
                          },
                          child: Text(l10n.historySearchApplyFilters),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Container(
      width: width,
      decoration: BoxDecoration(color: palette.navigation),
      foregroundDecoration: showDivider
          ? BoxDecoration(border: Border.all(color: palette.border))
          : null,
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
          child: _StickySidebarFrame(
            header: _ConversationSidebarHeader(sidebar: this),
            content: Stack(
              children: [
                Positioned.fill(
                  child: ListenableBuilder(
                    listenable: searchController,
                    builder: (context, _) {
                      final query = searchController.text.trim().toLowerCase();
                      final hasQuery = query.isNotEmpty;
                      if (isLoading || errorMessage != null) {
                        return ListView(
                          padding: const EdgeInsets.only(
                            top: 4,
                            left: 0,
                            right: 0,
                            bottom: 12,
                          ),
                          children: [
                            _SidebarSection(
                              title: l10n.chats,
                              emptyMessage: errorMessage ?? l10n.historyLoading,
                              onRetry: onRetryStorage,
                              onCreateConversation: onCreateConversation,
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
                      final visibleArchivedConversations = archivedConversations
                          .where(
                            (conversation) =>
                                !hasQuery ||
                                conversation.title.toLowerCase().contains(
                                  query,
                                ),
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
                            left: 0,
                            right: 0,
                            bottom: 12,
                          ),
                          children: [
                            if (conversations.isNotEmpty || selectionMode)
                              _buildBatchSelectionBar(context),
                            if (visiblePinnedConversations.isNotEmpty)
                              SidebarConversationSection(
                                title: l10n.pinnedChats,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noPinnedChats,
                                conversations: visiblePinnedConversations,
                                selectedConversationId: selectedConversationId,
                                itemHeight: 32,
                                dropIcon: LucideIcons.pin,
                                headingIcon: LucideIcons.pin,
                                onSelectConversation: onSelectConversation,
                                onTogglePinned: onToggleConversationPinned,
                                onRenameConversation: onRenameConversation,
                                onDeleteConversation: onDeleteConversation,
                                onExportConversation: onExportConversation,
                                onArchiveConversation: onArchiveConversation,
                                onEditConversationTags: onEditConversationTags,
                                onToggleConversationBookmark:
                                    onToggleConversationBookmark,
                                onDropConversation: onPinConversation,
                                selectionMode: selectionMode,
                                selectedConversationIds:
                                    selectedConversationIds,
                                onToggleBatchSelection: onToggleBatchSelection,
                                collapsed: _isCollapsed('pinned'),
                                onToggleCollapsed: _toggleSection('pinned'),
                              ),
                            if (visiblePinnedConversations.isNotEmpty)
                              const SizedBox(height: 16),
                            if (visibleProjects.isNotEmpty ||
                                !hasQuery ||
                                projectsLoading ||
                                projectLoadError != null)
                              ProjectSidebarSection(
                                projects: visibleProjects,
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
                                onArchiveConversation: onArchiveConversation,
                                onEditConversationTags: onEditConversationTags,
                                onToggleConversationBookmark:
                                    onToggleConversationBookmark,
                                selectionMode: selectionMode,
                                selectedConversationIds:
                                    selectedConversationIds,
                                onToggleBatchSelection: onToggleBatchSelection,
                                onCreateProject: onCreateProject,
                                loading: projectsLoading,
                                errorMessage: projectLoadError,
                                onRetry: onRetryProjects,
                                showEmptyMessage: false,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noProjects,
                              ),
                            if (visibleProjects.isNotEmpty &&
                                (visibleConversations.isNotEmpty || !hasQuery))
                              const SizedBox(height: 16),
                            if (visibleConversations.isNotEmpty || !hasQuery)
                              SidebarConversationSection(
                                title: l10n.chats,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noChatsTitle,
                                conversations: visibleConversations,
                                selectedConversationId: selectedConversationId,
                                itemHeight: 32,
                                dropIcon: LucideIcons.messageCircle,
                                headingIcon: LucideIcons.messagesSquare,
                                onSelectConversation: onSelectConversation,
                                onTogglePinned: onToggleConversationPinned,
                                onRenameConversation: onRenameConversation,
                                onDeleteConversation: onDeleteConversation,
                                onExportConversation: onExportConversation,
                                onArchiveConversation: onArchiveConversation,
                                onEditConversationTags: onEditConversationTags,
                                onToggleConversationBookmark:
                                    onToggleConversationBookmark,
                                onDropConversation: onMoveConversationToChats,
                                onCreateConversation: onCreateConversation,
                                selectionMode: selectionMode,
                                selectedConversationIds:
                                    selectedConversationIds,
                                onToggleBatchSelection: onToggleBatchSelection,
                                collapsed: _isCollapsed('chats'),
                                onToggleCollapsed: _toggleSection('chats'),
                              ),
                            if ((visiblePinnedConversations.isNotEmpty ||
                                    visibleProjects.isNotEmpty) &&
                                visibleArchivedConversations.isNotEmpty)
                              const SizedBox(height: 16),
                            if (visibleArchivedConversations.isNotEmpty)
                              SidebarConversationSection(
                                title: l10n.archivedChats,
                                emptyMessage: hasQuery
                                    ? l10n.noChatsSearchTitle
                                    : l10n.noArchivedChats,
                                conversations: visibleArchivedConversations,
                                selectedConversationId: selectedConversationId,
                                itemHeight: 32,
                                dropIcon: LucideIcons.archive,
                                headingIcon: LucideIcons.archive,
                                onSelectConversation: onSelectConversation,
                                onTogglePinned: null,
                                onRenameConversation: onRenameConversation,
                                onDeleteConversation: onDeleteConversation,
                                onExportConversation: onExportConversation,
                                onDropConversation: null,
                                isArchivedSection: true,
                                onArchiveConversation: onArchiveConversation,
                                onEditConversationTags: onEditConversationTags,
                                onToggleConversationBookmark:
                                    onToggleConversationBookmark,
                                collapsed: _isCollapsed('archived'),
                                onToggleCollapsed: _toggleSection('archived'),
                              ),
                          ],
                        );
                      }

                      if (!hasQuery) {
                        return ListView(
                          padding: const EdgeInsets.only(
                            top: 4,
                            left: 0,
                            right: 0,
                            bottom: 12,
                          ),
                          children: [
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
                              onRetry: onRetryProjects,
                              showEmptyMessage: false,
                              emptyMessage: hasQuery
                                  ? l10n.noChatsSearchTitle
                                  : l10n.noProjects,
                            ),
                            SidebarConversationSection(
                              title: l10n.chats,
                              emptyMessage: l10n.noChatsTitle,
                              conversations: const [],
                              selectedConversationId: selectedConversationId,
                              itemHeight: 32,
                              dropIcon: LucideIcons.messageCircle,
                              headingIcon: LucideIcons.messagesSquare,
                              onSelectConversation: onSelectConversation,
                              onTogglePinned: onToggleConversationPinned,
                              onRenameConversation: onRenameConversation,
                              onDeleteConversation: onDeleteConversation,
                              onExportConversation: onExportConversation,
                              onDropConversation: onMoveConversationToChats,
                              onCreateConversation: onCreateConversation,
                              collapsed: _isCollapsed('chats'),
                              onToggleCollapsed: _toggleSection('chats'),
                            ),
                          ],
                        );
                      }
                      return ListView(
                        padding: const EdgeInsets.only(
                          top: 4,
                          left: 0,
                          right: 0,
                          bottom: 12,
                        ),
                        children: [
                          _SidebarSection(
                            title: l10n.chats,
                            emptyMessage: l10n.noChatsSearchTitle,
                            onCreateConversation: onCreateConversation,
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
      ),
    );
  }
}

class _ConversationSidebarHeader extends StatefulWidget {
  const _ConversationSidebarHeader({required this.sidebar});

  final ConversationSidebar sidebar;

  @override
  State<_ConversationSidebarHeader> createState() =>
      _ConversationSidebarHeaderState();
}

class _ConversationSidebarHeaderState
    extends State<_ConversationSidebarHeader> {
  bool _isSearchDialogOpen = false;
  bool _isSearchDialogScheduled = false;

  @override
  void initState() {
    super.initState();
    _scheduleSearchDialog(widget.sidebar.isHistorySearchOpen);
  }

  @override
  void didUpdateWidget(covariant _ConversationSidebarHeader oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!oldWidget.sidebar.isHistorySearchOpen &&
        widget.sidebar.isHistorySearchOpen) {
      _scheduleSearchDialog(true);
    }
  }

  void _scheduleSearchDialog(bool isOpen) {
    if (!isOpen || _isSearchDialogOpen || _isSearchDialogScheduled) return;
    _isSearchDialogScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _isSearchDialogScheduled = false;
      if (mounted && widget.sidebar.isHistorySearchOpen) {
        unawaited(_showSearchDialog());
      }
    });
  }

  Future<void> _showSearchDialog() async {
    if (_isSearchDialogOpen) return;
    _isSearchDialogOpen = true;
    final result = await showDialog<Object>(
      context: context,
      animationStyle: MediaQuery.disableAnimationsOf(context)
          ? AnimationStyle.noAnimation
          : null,
      builder: (dialogContext) {
        final revision = widget.sidebar.historySearchDialogRevision;
        if (revision == null) return _buildSearchDialog(dialogContext);
        return AnimatedBuilder(
          animation: revision,
          builder: (context, _) => _buildSearchDialog(context),
        );
      },
    );
    if (!mounted) return;
    _isSearchDialogOpen = false;
    final sidebar = widget.sidebar;
    if (result == null) {
      sidebar.onCloseHistorySearch?.call();
    } else if (result is HistorySearchResult) {
      sidebar.onSelectHistoryResult?.call(result);
    } else if (result is LocalLibrarySearchResult) {
      sidebar.onSelectLocalLibraryResult?.call(result);
    }
  }

  Widget _buildSearchDialog(BuildContext context) {
    final sidebar = widget.sidebar;
    return _HistorySearchDialog(
      sidebar: sidebar,
      onOpenFilters: () => sidebar._showHistorySearchFilters(context),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return ConstrainedBox(
      key: const ValueKey<String>('conversation-sidebar-header'),
      constraints: const BoxConstraints(
        minHeight: OpenChatSpacing.conversationHeaderHeight,
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          6,
          OpenChatSpacing.xs,
          6,
          OpenChatSpacing.xs,
        ),
        child: Row(
          children: [
            Flexible(
              child: FilledButton.tonalIcon(
                onPressed: widget.sidebar.onCreateConversation,
                icon: const Icon(LucideIcons.plus, size: 17),
                label: Text(
                  l10n.newChat,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                style: FilledButton.styleFrom(
                  minimumSize: const Size(0, 40),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                ),
              ),
            ),
            const SizedBox(width: 8),
            IconButton(
              tooltip: l10n.temporaryChatStart,
              onPressed: widget.sidebar.onCreateTemporaryConversation,
              visualDensity: VisualDensity.compact,
              constraints: const BoxConstraints.tightFor(width: 40, height: 40),
              padding: EdgeInsets.zero,
              color: palette.secondaryIcon,
              icon: const Icon(LucideIcons.timer, size: 18),
            ),
            const SizedBox(width: 4),
            IconButton(
              tooltip: l10n.searchMessagesTooltip,
              onPressed: widget.sidebar.onOpenHistorySearch,
              visualDensity: VisualDensity.compact,
              constraints: const BoxConstraints.tightFor(width: 40, height: 40),
              padding: EdgeInsets.zero,
              color: palette.secondaryIcon,
              icon: const Icon(LucideIcons.search, size: 18),
            ),
          ],
        ),
      ),
    );
  }
}

class _HistorySearchDialog extends StatelessWidget {
  const _HistorySearchDialog({
    required this.sidebar,
    required this.onOpenFilters,
  });

  final ConversationSidebar sidebar;
  final VoidCallback onOpenFilters;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final dialogStartInset = sidebar.showDivider
        ? OpenChatSpacing.compactRailWidth
        : 0.0;
    final dialogWidth = (sidebar.width - OpenChatSpacing.md)
        .clamp(0.0, 780.0)
        .toDouble();

    return Dialog(
      alignment: AlignmentDirectional.centerStart,
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shadowColor: Colors.transparent,
      insetPadding: EdgeInsets.fromLTRB(
        dialogStartInset + OpenChatSpacing.sm,
        OpenChatSpacing.md,
        OpenChatSpacing.md,
        OpenChatSpacing.md,
      ),
      child: OpenChatGlassSurface(
        borderRadius: BorderRadius.circular(OpenChatRadii.dialog),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 780, maxHeight: 680),
          child: SizedBox(
            width: dialogWidth,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(
                    OpenChatSpacing.md,
                    OpenChatSpacing.md,
                    OpenChatSpacing.sm,
                    OpenChatSpacing.xs,
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            ExcludeSemantics(
                              child: Padding(
                                padding: const EdgeInsetsDirectional.only(
                                  start: 2,
                                  bottom: OpenChatSpacing.xxs,
                                ),
                                child: Text(
                                  l10n.searchChats,
                                  style: theme.textTheme.bodySmall?.copyWith(
                                    color: palette.secondaryText,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ),
                            ),
                            Semantics(
                              textField: true,
                              label: l10n.searchChats,
                              child: shad.ShadInput(
                                key: const ValueKey<String>(
                                  'history-search-input',
                                ),
                                autofocus: true,
                                controller: sidebar.searchController,
                                onChanged: sidebar.onSearchChanged,
                                onSubmitted: sidebar.onSearchSubmitted,
                                textInputAction: TextInputAction.search,
                                cursorColor: palette.focusRing,
                                style: theme.textTheme.bodyMedium?.copyWith(
                                  color: palette.text,
                                ),
                                placeholder: ExcludeSemantics(
                                  child: Text(l10n.searchChatsHint),
                                ),
                                placeholderStyle: theme.textTheme.bodyMedium
                                    ?.copyWith(color: palette.secondaryText),
                                leading: Icon(
                                  LucideIcons.search,
                                  size: 18,
                                  color: palette.secondaryIcon,
                                ),
                                trailing: AnimatedSwitcher(
                                  duration: reducedMotion
                                      ? Duration.zero
                                      : OpenChatMotion.quick,
                                  child: sidebar.isHistorySearchLoading
                                      ? Semantics(
                                          key: ValueKey<String>(
                                            reducedMotion
                                                ? 'history-search-loading-static'
                                                : 'history-search-loading',
                                          ),
                                          liveRegion: true,
                                          label: l10n.searchMessagesLoading,
                                          child: reducedMotion
                                              ? const Icon(
                                                  LucideIcons.loaderCircle,
                                                  size: 18,
                                                )
                                              : const SizedBox(
                                                  width: 24,
                                                  height: 24,
                                                  child: Center(
                                                    child: SizedBox.square(
                                                      dimension: 16,
                                                      child:
                                                          CircularProgressIndicator(
                                                            strokeWidth: 2,
                                                          ),
                                                    ),
                                                  ),
                                                ),
                                        )
                                      : const SizedBox.shrink(
                                          key: ValueKey<String>(
                                            'history-search-idle',
                                          ),
                                        ),
                                ),
                                padding: const EdgeInsets.symmetric(
                                  horizontal: OpenChatSpacing.sm,
                                  vertical: OpenChatSpacing.xs,
                                ),
                                constraints: const BoxConstraints(
                                  minHeight: 40,
                                ),
                                decoration: shad.ShadDecoration(
                                  color: palette.hover,
                                  border: shad.ShadBorder.all(
                                    color: palette.controlBorder,
                                    radius: BorderRadius.circular(
                                      OpenChatRadii.control,
                                    ),
                                  ),
                                  focusedBorder: shad.ShadBorder.all(
                                    color: palette.focusRing,
                                    width: 2,
                                    radius: BorderRadius.circular(
                                      OpenChatRadii.control,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(width: OpenChatSpacing.xs),
                      IconButton(
                        tooltip: l10n.close,
                        onPressed: () => Navigator.of(context).pop(),
                        icon: const Icon(LucideIcons.x),
                      ),
                    ],
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: OpenChatSpacing.md,
                  ),
                  child: Wrap(
                    alignment: WrapAlignment.end,
                    spacing: OpenChatSpacing.xxs,
                    runSpacing: OpenChatSpacing.xxs,
                    children: [
                      IconButton(
                        tooltip: sidebar.hasHistorySearchDateRange
                            ? l10n.historySearchDateFilterApplied
                            : l10n.historySearchDateFilter,
                        onPressed: sidebar.onPickHistorySearchDateRange,
                        visualDensity: VisualDensity.compact,
                        constraints: const BoxConstraints.tightFor(
                          width: 40,
                          height: 40,
                        ),
                        padding: EdgeInsets.zero,
                        icon: Icon(
                          LucideIcons.calendarDays,
                          color: sidebar.hasHistorySearchDateRange
                              ? palette.accent
                              : palette.secondaryIcon,
                          size: 17,
                        ),
                      ),
                      if (sidebar.onOpenSavedHistorySearches != null)
                        IconButton(
                          tooltip: l10n.savedHistorySearchesTitle,
                          onPressed: sidebar.onOpenSavedHistorySearches,
                          visualDensity: VisualDensity.compact,
                          constraints: const BoxConstraints.tightFor(
                            width: 40,
                            height: 40,
                          ),
                          padding: EdgeInsets.zero,
                          icon: Icon(
                            LucideIcons.bookmark,
                            color: palette.secondaryIcon,
                            size: 17,
                          ),
                        ),
                      IconButton(
                        tooltip: sidebar.historySearchFilters.isEmpty
                            ? l10n.historySearchOpenFilters
                            : l10n.historySearchFiltersActive,
                        onPressed: onOpenFilters,
                        visualDensity: VisualDensity.compact,
                        constraints: const BoxConstraints.tightFor(
                          width: 40,
                          height: 40,
                        ),
                        padding: EdgeInsets.zero,
                        icon: Icon(
                          LucideIcons.slidersHorizontal,
                          color: sidebar.historySearchFilters.isEmpty
                              ? palette.secondaryIcon
                              : palette.accent,
                          size: 17,
                        ),
                      ),
                      if (sidebar.hasHistorySearchDateRange)
                        IconButton(
                          tooltip: l10n.historySearchClearDateFilter,
                          onPressed: sidebar.onClearHistorySearchDateRange,
                          visualDensity: VisualDensity.compact,
                          constraints: const BoxConstraints.tightFor(
                            width: 40,
                            height: 40,
                          ),
                          padding: EdgeInsets.zero,
                          icon: Icon(
                            LucideIcons.x,
                            color: palette.secondaryIcon,
                            size: 16,
                          ),
                        ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(
                      OpenChatSpacing.lg,
                      OpenChatSpacing.md,
                      OpenChatSpacing.lg,
                      OpenChatSpacing.lg,
                    ),
                    child: _HistorySearchResultsSection(
                      query: sidebar.searchController.text.trim(),
                      results: sidebar.historySearchResults,
                      localResults: sidebar.localLibrarySearchResults,
                      filter: sidebar.unifiedLibrarySearchFilter,
                      loading: sidebar.isHistorySearchLoading,
                      errorCode: sidebar.historySearchErrorCode,
                      onSearch: sidebar.onSearchSubmitted,
                      onSelectResult: (result) =>
                          Navigator.of(context).pop(result),
                      onSelectLocalResult: (result) =>
                          Navigator.of(context).pop(result),
                      onFilterChanged:
                          sidebar.onUnifiedLibrarySearchFilterChanged,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _StickySidebarFrame extends StatefulWidget {
  const _StickySidebarFrame({required this.header, required this.content});

  final Widget header;
  final Widget content;

  @override
  State<_StickySidebarFrame> createState() => _StickySidebarFrameState();
}

class _StickySidebarFrameState extends State<_StickySidebarFrame> {
  bool _isScrolled = false;

  bool _handleScroll(ScrollNotification notification) {
    if (notification.metrics.axis != Axis.vertical) {
      return false;
    }
    final isScrolled = notification.metrics.pixels > 0;
    if (_isScrolled != isScrolled) {
      setState(() => _isScrolled = isScrolled);
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final palette = OpenChatPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        DecoratedBox(
          decoration: BoxDecoration(
            color: palette.navigation,
            border: Border(
              bottom: BorderSide(
                color: _isScrolled ? palette.border : Colors.transparent,
              ),
            ),
          ),
          child: widget.header,
        ),
        Expanded(
          child: NotificationListener<ScrollNotification>(
            onNotification: _handleScroll,
            child: widget.content,
          ),
        ),
      ],
    );
  }
}

class _HistorySearchResultsSection extends StatefulWidget {
  const _HistorySearchResultsSection({
    required this.query,
    required this.results,
    required this.localResults,
    required this.filter,
    required this.loading,
    required this.errorCode,
    required this.onSearch,
    required this.onSelectResult,
    required this.onSelectLocalResult,
    required this.onFilterChanged,
  });

  final String query;
  final List<HistorySearchResult> results;
  final List<LocalLibrarySearchResult> localResults;
  final UnifiedLibrarySearchFilter filter;
  final bool loading;
  final String? errorCode;
  final ValueChanged<String>? onSearch;
  final ValueChanged<HistorySearchResult>? onSelectResult;
  final ValueChanged<LocalLibrarySearchResult>? onSelectLocalResult;
  final ValueChanged<UnifiedLibrarySearchFilter>? onFilterChanged;

  @override
  State<_HistorySearchResultsSection> createState() =>
      _HistorySearchResultsSectionState();
}

class _HistorySearchResultsSectionState
    extends State<_HistorySearchResultsSection> {
  late UnifiedLibrarySearchFilter _filter = widget.filter;

  @override
  void didUpdateWidget(covariant _HistorySearchResultsSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.filter != widget.filter) _filter = widget.filter;
  }

  @override
  Widget build(BuildContext context) {
    final query = widget.query;
    final results = widget.results;
    final localResults = widget.localResults;
    final filter = _filter;
    final loading = widget.loading;
    final errorCode = widget.errorCode;
    final onSearch = widget.onSearch;
    final onSelectResult = widget.onSelectResult;
    final onSelectLocalResult = widget.onSelectLocalResult;
    final onFilterChanged = widget.onFilterChanged;
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    final message = switch (errorCode) {
      'too_short' => l10n.searchMessagesTooShort,
      'too_long' => l10n.searchMessagesTooLong,
      'failed' => l10n.searchMessagesFailed,
      _ => null,
    };
    final visibleHistoryResults =
        filter == UnifiedLibrarySearchFilter.all ||
            filter == UnifiedLibrarySearchFilter.chats
        ? results
        : const <HistorySearchResult>[];
    final visibleLocalResults = localResults
        .where((result) {
          return switch (filter) {
            UnifiedLibrarySearchFilter.all => true,
            UnifiedLibrarySearchFilter.chats => false,
            UnifiedLibrarySearchFilter.projects =>
              result.kind == LocalLibrarySearchResultKind.project,
            UnifiedLibrarySearchFilter.savedResponses =>
              result.kind == LocalLibrarySearchResultKind.savedResponse,
          };
        })
        .toList(growable: false);
    final searchFilters = <(UnifiedLibrarySearchFilter, String)>[
      (UnifiedLibrarySearchFilter.all, l10n.searchFilterAll),
      (UnifiedLibrarySearchFilter.chats, l10n.searchFilterChats),
      (UnifiedLibrarySearchFilter.projects, l10n.searchFilterProjects),
      (
        UnifiedLibrarySearchFilter.savedResponses,
        l10n.searchFilterSavedResponses,
      ),
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 4,
          children: [
            for (final (value, label) in searchFilters)
              ChoiceChip(
                label: Text(label),
                selected: filter == value,
                onSelected: (_) {
                  setState(() => _filter = value);
                  onFilterChanged?.call(value);
                },
              ),
          ],
        ),
        const SizedBox(height: OpenChatSpacing.xs),
        if (loading)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
            child: Semantics(
              liveRegion: true,
              label: l10n.searchMessagesLoading,
              child: ExcludeSemantics(
                child: reducedMotion
                    ? Row(
                        children: [
                          const Icon(LucideIcons.loaderCircle, size: 16),
                          const SizedBox(width: OpenChatSpacing.xs),
                          Expanded(child: Text(l10n.searchMessagesLoading)),
                        ],
                      )
                    : const LinearProgressIndicator(minHeight: 2),
              ),
            ),
          )
        else if (message != null &&
            visibleHistoryResults.isEmpty &&
            visibleLocalResults.isEmpty)
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
                    onPressed: () => onSearch(query),
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(LucideIcons.refreshCw, size: 17),
                  ),
              ],
            ),
          )
        else if (query.isEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(4, 4, 4, 8),
            child: Text(
              l10n.searchMessagesTooShort,
              style: theme.textTheme.bodySmall?.copyWith(
                color: palette.secondaryText,
              ),
            ),
          )
        else if (visibleHistoryResults.isEmpty && visibleLocalResults.isEmpty)
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
          for (final result in visibleHistoryResults)
            Material(
              key: ValueKey<String>(
                '${result.conversationId}:${result.messageId ?? 'title'}',
              ),
              color: Colors.transparent,
              child: InkWell(
                onTap: onSelectResult == null
                    ? null
                    : () => onSelectResult(result),
                mouseCursor: SystemMouseCursors.click,
                hoverColor: palette.hover,
                focusColor: palette.hover,
                borderRadius: BorderRadius.circular(OpenChatRadii.button),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 5,
                    vertical: 7,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Row(
                        children: [
                          if (result.isConversationTitleMatch)
                            Padding(
                              padding: const EdgeInsets.only(right: 7),
                              child: Icon(
                                LucideIcons.messagesSquare,
                                size: 15,
                                color: palette.secondaryText,
                              ),
                            )
                          else
                            const SizedBox(width: 22),
                          Expanded(
                            child: Text(
                              result.conversationTitle,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: palette.text,
                                fontWeight: FontWeight.w500,
                              ),
                            ),
                          ),
                        ],
                      ),
                      if (!result.isConversationTitleMatch)
                        Padding(
                          padding: const EdgeInsets.only(left: 30, top: 4),
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              Container(
                                width: 1,
                                margin: const EdgeInsets.symmetric(vertical: 2),
                                color: palette.border,
                              ),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text.rich(
                                  TextSpan(
                                    children: _historyExcerptSpans(
                                      result.excerpt,
                                      normalStyle: theme.textTheme.bodySmall
                                          ?.copyWith(
                                            color: palette.secondaryText,
                                          ),
                                      matchStyle: theme.textTheme.bodySmall
                                          ?.copyWith(
                                            color: theme
                                                .colorScheme
                                                .onPrimaryContainer,
                                            backgroundColor: theme
                                                .colorScheme
                                                .primaryContainer,
                                            fontWeight: FontWeight.w600,
                                          ),
                                    ),
                                  ),
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            ),
        for (final result in visibleLocalResults)
          _LocalLibrarySearchResultTile(
            result: result,
            onTap: result.conversationId == null || onSelectLocalResult == null
                ? null
                : () => onSelectLocalResult(result),
          ),
      ],
    );
  }
}

class _LocalLibrarySearchResultTile extends StatelessWidget {
  const _LocalLibrarySearchResultTile({required this.result, this.onTap});

  final LocalLibrarySearchResult result;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final theme = Theme.of(context);
    final kindLabel = switch (result.kind) {
      LocalLibrarySearchResultKind.project => l10n.projects,
      LocalLibrarySearchResultKind.savedResponse => l10n.outputs,
    };
    final icon = switch (result.kind) {
      LocalLibrarySearchResultKind.project => LucideIcons.folder,
      LocalLibrarySearchResultKind.savedResponse => LucideIcons.bookmark,
    };

    return Material(
      key: ValueKey<String>('local-library-${result.kind.name}-${result.id}'),
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        mouseCursor: onTap == null
            ? SystemMouseCursors.basic
            : SystemMouseCursors.click,
        hoverColor: palette.hover,
        focusColor: palette.hover,
        borderRadius: BorderRadius.circular(OpenChatRadii.button),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 7),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Padding(
                padding: const EdgeInsetsDirectional.only(end: 8, top: 2),
                child: Icon(icon, size: 15, color: palette.secondaryText),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Text(
                      result.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: palette.text,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    if (result.subtitle case final subtitle?) ...[
                      const SizedBox(height: 3),
                      Text(
                        subtitle,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: palette.secondaryText,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: OpenChatSpacing.xs),
              Text(
                kindLabel,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: palette.secondaryText,
                ),
              ),
            ],
          ),
        ),
      ),
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
    this.onCreateConversation,
  });

  final String title;
  final String emptyMessage;
  final VoidCallback? onRetry;
  final VoidCallback? onCreateConversation;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final touch = switch (Theme.of(context).platform) {
      TargetPlatform.android ||
      TargetPlatform.iOS ||
      TargetPlatform.fuchsia => true,
      _ => false,
    };
    final palette = OpenChatPalette.of(context);

    return ConstrainedBox(
      constraints: BoxConstraints(minHeight: onRetry == null ? 56 : 104),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: SidebarSectionHeading(title: title)),
              if (onCreateConversation case final createConversation?)
                IconButton(
                  key: const ValueKey<String>('conversations-create-button'),
                  tooltip: context.openchatL10n.newChat,
                  onPressed: createConversation,
                  visualDensity: VisualDensity.standard,
                  constraints: BoxConstraints.tightFor(
                    width: touch ? 44 : 32,
                    height: touch ? 44 : 32,
                  ),
                  padding: EdgeInsets.zero,
                  icon: const Icon(LucideIcons.plus, size: 19),
                ),
            ],
          ),
          const SizedBox(height: 8),
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
