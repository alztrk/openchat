import 'package:drift/native.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/outputs_page.dart';
import 'package:openchat/features/chat/presentation/workspaces_page.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final lucideFont = FontLoader('packages/lucide_icons_flutter/Lucide')
      ..addFont(
        rootBundle.load('packages/lucide_icons_flutter/assets/lucide.ttf'),
      );
    await lucideFont.load();
    final uiFont = FontLoader(OpenChatTypography.uiFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceSans3VF-Upright.ttf'));
    await uiFont.load();
    final codeFont = FontLoader(OpenChatTypography.codeFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceCodeVF-Upright.ttf'));
    await codeFont.load();
  });

  testWidgets('creates a workspace and starts a chat inside it', (
    tester,
  ) async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    String? openedWorkspaceId;
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));

    await tester.pumpWidget(
      _testApp(
        WorkspacesPage(
          repository: repository,
          onCreateWorkspace: (name) => repository.createWorkspace(
            id: 'workspace-1',
            name: name,
            createdAt: DateTime.utc(2026, 10, 9),
          ),
          onRenameWorkspace: (workspaceId, name) =>
              repository.renameWorkspace(workspaceId: workspaceId, name: name),
          onDeleteWorkspace: repository.deleteWorkspace,
          onSetConversationWorkspace: (conversationId, workspaceId) =>
              repository.setConversationWorkspace(
                conversationId: conversationId,
                workspaceId: workspaceId,
              ),
          onCreateConversation: (workspaceId) =>
              openedWorkspaceId = workspaceId,
          onOpenConversation: (_) {},
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text(l10n.workspaceEmptyTitle), findsOneWidget);
    await tester.tap(find.text(l10n.workspaceCreate).first);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Araştırma');
    await tester.tap(find.text(l10n.workspaceCreateAction).last);
    await tester.pumpAndSettle();

    expect(find.text('Araştırma'), findsOneWidget);
    await tester.tap(find.text(l10n.newChat));
    expect(openedWorkspaceId, 'workspace-1');
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets('shows, opens, and removes a saved assistant response', (
    tester,
  ) async {
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 10, 9, 12);
    await repository.createConversation(
      id: 'conversation-1',
      title: 'Araştırma notları',
      createdAt: createdAt,
    );
    await repository.saveMessage(
      conversationId: 'conversation-1',
      message: ChatMessage(
        id: 'answer-1',
        role: ChatMessageRole.assistant,
        content: 'Kaydedilmiş yanıt',
        createdAt: createdAt,
        status: ChatMessageStatus.completed,
      ),
    );
    await repository.saveOutput(
      conversationId: 'conversation-1',
      messageId: 'answer-1',
    );
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));
    String? openedMessageId;

    await tester.pumpWidget(
      _testApp(
        OutputsPage(
          repository: repository,
          onOpenConversation: (output) => openedMessageId = output.messageId,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Araştırma notları'), findsOneWidget);
    expect(find.text('Kaydedilmiş yanıt'), findsOneWidget);
    await tester.tap(find.text(l10n.outputOpenConversation));
    expect(openedMessageId, 'answer-1');

    await tester.tap(find.byTooltip(l10n.removeSavedResponse));
    await tester.pumpAndSettle();
    expect(find.text(l10n.outputsEmptyTitle), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 1));
  });
}

Widget _testApp(Widget child) => MaterialApp(
  locale: const Locale('tr'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: OpenChatTheme.light,
  home: Scaffold(body: child),
);
