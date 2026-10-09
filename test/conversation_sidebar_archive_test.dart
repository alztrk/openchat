import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/domain/history_search_result.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:openchat/features/chat/presentation/widgets/sidebar_conversation_tile.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

import 'support/shad_test_scope.dart';

void main() {
  testWidgets('shows archived conversations with a restore action', (
    tester,
  ) async {
    var restoredConversationId = '';
    final searchController = TextEditingController();
    addTearDown(searchController.dispose);

    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationSidebar(
            width: 300,
            searchController: searchController,
            conversations: const <ConversationSidebarConversation>[
              ConversationSidebarConversation(
                id: 'active',
                title: 'Active chat',
              ),
            ],
            archivedConversations: const <ConversationSidebarConversation>[
              ConversationSidebarConversation(
                id: 'archived',
                title: 'Archived chat',
                isArchived: true,
              ),
            ],
            selectedConversationId: 'archived',
            onArchiveConversation: (id) => restoredConversationId = id,
          ),
        ),
      ),
    );

    expect(find.text('Archived'), findsOneWidget);
    expect(find.text('Archived chat'), findsOneWidget);
    await tester.tap(find.byTooltip('More options').last);
    await tester.pumpAndSettle();
    expect(find.text('Restore conversation'), findsOneWidget);
    await tester.tap(find.text('Restore conversation'));
    await tester.pumpAndSettle();
    expect(restoredConversationId, 'archived');
  });

  testWidgets('selects active conversations for batch archive', (tester) async {
    final selected = <String>{};
    var archiveRequested = false;
    final searchController = TextEditingController();
    addTearDown(searchController.dispose);
    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationSidebar(
            width: 320,
            searchController: searchController,
            conversations: const <ConversationSidebarConversation>[
              ConversationSidebarConversation(id: 'batch-a', title: 'Batch A'),
            ],
            onToggleSelectionMode: () {},
            selectionMode: true,
            selectedConversationIds: selected,
            onToggleBatchSelection: selected.add,
            onArchiveSelected: () => archiveRequested = true,
          ),
        ),
      ),
    );

    expect(find.text('1 chat selected'), findsNothing);
    expect(find.byType(Checkbox), findsOneWidget);
    expect(find.bySemanticsLabel('Select Batch A'), findsOneWidget);
    await tester.ensureVisible(find.byType(Checkbox));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(Checkbox));
    await tester.pump();
    expect(selected, <String>{'batch-a'});
    expect(find.byTooltip('Archive selected chats'), findsOneWidget);
    expect(archiveRequested, isFalse);
  });

  testWidgets('shows tags in accessible conversation names and edit menu', (
    tester,
  ) async {
    var editedConversationId = '';
    final searchController = TextEditingController();
    addTearDown(searchController.dispose);
    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationSidebar(
            width: 280,
            searchController: searchController,
            conversations: const <ConversationSidebarConversation>[
              ConversationSidebarConversation(
                id: 'tagged',
                title: 'Tagged chat',
                tags: <String>['Research', 'Follow up'],
              ),
            ],
            onEditConversationTags: (id) => editedConversationId = id,
          ),
        ),
      ),
    );
    final tileSemantics = tester.widget<Semantics>(
      find
          .descendant(
            of: find.byType(SidebarConversationTile),
            matching: find.byType(Semantics),
          )
          .first,
    );
    expect(tileSemantics.properties.label, 'Tagged chat, Research, Follow up');
    await tester.tap(find.byTooltip('More options'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Edit tags'));
    await tester.pumpAndSettle();
    expect(editedConversationId, 'tagged');
  });

  testWidgets('offers a bookmark action for a conversation', (tester) async {
    var bookmarkedConversationId = '';
    final searchController = TextEditingController();
    addTearDown(searchController.dispose);
    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationSidebar(
            width: 320,
            searchController: searchController,
            conversations: const <ConversationSidebarConversation>[
              ConversationSidebarConversation(
                id: 'bookmark',
                title: 'Bookmark me',
              ),
            ],
            onToggleConversationBookmark: (id) => bookmarkedConversationId = id,
          ),
        ),
      ),
    );
    await tester.tap(find.byTooltip('More options'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Bookmark conversation'));
    await tester.pumpAndSettle();
    expect(bookmarkedConversationId, 'bookmark');
  });

  testWidgets('applies the selected conversation tag to history search', (
    tester,
  ) async {
    HistorySearchFilters? appliedFilters;
    final searchController = TextEditingController();
    addTearDown(searchController.dispose);

    await tester.pumpWidget(
      MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
        home: Scaffold(
          body: ConversationSidebar(
            width: 320,
            searchController: searchController,
            isHistorySearchOpen: true,
            historySearchFilterOptions: const HistorySearchFilterOptions(
              providerIds: <String>[],
              modelIds: <String>[],
              tags: <String>['Research'],
            ),
            onApplyHistorySearchFilters: (filters) => appliedFilters = filters,
          ),
        ),
      ),
    );

    await tester.tap(find.byTooltip('Open search filters'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('All tags'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Research').last);
    await tester.ensureVisible(find.text('Apply filters'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Apply filters'));
    await tester.pumpAndSettle();

    expect(appliedFilters?.tag, 'Research');
  });
}
