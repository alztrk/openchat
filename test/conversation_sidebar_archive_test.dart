import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_sidebar.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

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
}
