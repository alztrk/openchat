import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/l10n/openchat_localizations.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/presentation/widgets/project_sidebar_section.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_section.dart';

class ConversationSidebar extends StatelessWidget {
  const ConversationSidebar({
    required this.searchController,
    this.isHistorySearchOpen = false,
    this.onOpenHistorySearch,
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
  final bool isHistorySearchOpen;
  final VoidCallback? onOpenHistorySearch;
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
              border: Border(
                right: BorderSide(
                  color: palette.border.withValues(alpha: 0.48),
                ),
              ),
            )
          : null,
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                key: const ValueKey<String>('conversation-sidebar-header'),
                height: OpenChatSpacing.conversationHeaderHeight,
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
                                              child: CircularProgressIndicator(
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
                                  l10n.chats,
                                  style: Theme.of(context).textTheme.titleLarge,
                                ),
                              ),
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
                        width: 28,
                        height: 28,
                      ),
                      padding: EdgeInsets.zero,
                      icon: isHistorySearchOpen
                          ? Icon(
                              Icons.close_rounded,
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
              Divider(height: 1, color: palette.border.withValues(alpha: 0.48)),
              const SizedBox(height: 16),
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
                                right: 8,
                                bottom: 16,
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

                          final hasTitleResults =
                              visibleProjects.isNotEmpty ||
                              visiblePinnedConversations.isNotEmpty ||
                              visibleConversations.isNotEmpty;
                          if (hasTitleResults) {
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
                                  selectedConversationId:
                                      selectedConversationId,
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
                                  selectedConversationId:
                                      selectedConversationId,
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
                    if (isHistorySearchOpen &&
                        searchController.text.trim().isNotEmpty &&
                        historySearchQuery?.trim().toLowerCase() ==
                            searchController.text.trim().toLowerCase())
                      Positioned(
                        top: 0,
                        left: 0,
                        right: 8,
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
                    icon: const Icon(Icons.refresh_rounded, size: 17),
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
  });

  final String title;
  final String emptyMessage;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return ConstrainedBox(
      constraints: BoxConstraints(minHeight: onRetry == null ? 56 : 104),
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
          Padding(
            padding: EdgeInsets.only(bottom: onRetry == null ? 0 : 2),
            child: Text(
              emptyMessage,
              maxLines: onRetry == null ? 1 : 2,
              overflow: TextOverflow.ellipsis,
              style: textTheme.bodyMedium?.copyWith(
                fontSize: 13,
                fontWeight: FontWeight.w400,
                height: 18 / 13,
              ),
            ),
          ),
          if (onRetry case final retry?)
            TextButton.icon(
              onPressed: retry,
              icon: const Icon(Icons.refresh_rounded, size: 16),
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
