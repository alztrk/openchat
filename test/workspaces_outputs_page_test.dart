import 'package:drift/native.dart';
import 'package:drift/drift.dart' show Value;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/outputs_page.dart';
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
        providerId: 'chatgpt',
        modelId: 'gpt-5.5',
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
    expect(
      find.textContaining(l10n.outputSourceProvider(l10n.chatGptProvider)),
      findsOneWidget,
    );
    expect(
      find.textContaining(l10n.outputSourceModel('gpt-5.5')),
      findsOneWidget,
    );
    await tester.tap(find.text(l10n.outputOpenConversation));
    expect(openedMessageId, 'answer-1');

    await tester.tap(find.byTooltip(l10n.removeSavedResponse));
    await tester.pumpAndSettle();
    expect(find.text(l10n.outputsEmptyTitle), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets('opens the response workspace with the saved source copy', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(420, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final database = OpenChatDatabase(NativeDatabase.memory());
    addTearDown(database.close);
    final repository = ChatRepository(database);
    final createdAt = DateTime.utc(2026, 10, 9, 12);
    await repository.createConversation(
      id: 'conversation-workspace',
      title: 'Yanıt çalışma alanı',
      createdAt: createdAt,
    );
    await repository.saveMessage(
      conversationId: 'conversation-workspace',
      message: ChatMessage(
        id: 'answer-workspace',
        role: ChatMessageRole.assistant,
        content: 'Kaynak yanıt',
        createdAt: createdAt,
        status: ChatMessageStatus.completed,
      ),
    );
    await repository.saveOutput(
      conversationId: 'conversation-workspace',
      messageId: 'answer-workspace',
    );
    final l10n = await AppLocalizations.delegate.load(const Locale('tr'));

    await tester.pumpWidget(
      _testApp(OutputsPage(repository: repository, onOpenConversation: (_) {})),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text(l10n.outputWorkspaceOpen));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.text(l10n.outputWorkspaceTitle).last, findsOneWidget);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'Kaynak yanıt',
    );
    await tester.tap(find.byTooltip(l10n.outputWorkspaceExport));
    await tester.pumpAndSettle();
    expect(find.text(l10n.outputExportReviewTitle), findsOneWidget);
    expect(find.text(l10n.outputExportReviewScope(1)), findsOneWidget);
    expect(find.text(l10n.outputExportReviewSensitiveNotice), findsOneWidget);
    expect(find.text('Kaynak yanıt'), findsWidgets);
    await tester.tap(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.text(l10n.cancel),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.tap(find.text(l10n.cancel));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(TextField), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets(
    'saves workspace edits as a version without changing source chat',
    (tester) async {
      final database = OpenChatDatabase(NativeDatabase.memory());
      addTearDown(database.close);
      final repository = ChatRepository(database);
      final createdAt = DateTime.utc(2026, 10, 9, 12);
      await repository.createConversation(
        id: 'conversation-edit',
        title: 'Edited response',
        createdAt: createdAt,
      );
      await repository.saveMessage(
        conversationId: 'conversation-edit',
        message: ChatMessage(
          id: 'answer-edit',
          role: ChatMessageRole.assistant,
          content: '# Original response\n\n```dart\nfinal value = 1;\n```',
          createdAt: createdAt,
          status: ChatMessageStatus.completed,
        ),
      );
      await repository.saveOutput(
        conversationId: 'conversation-edit',
        messageId: 'answer-edit',
      );
      final l10n = await AppLocalizations.delegate.load(const Locale('tr'));

      await tester.pumpWidget(
        _testApp(
          OutputsPage(repository: repository, onOpenConversation: (_) {}),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text(l10n.outputWorkspaceOpen));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      await tester.tap(find.text(l10n.outputWorkspacePreview));
      await tester.pumpAndSettle();
      expect(find.byType(HtmlWidget), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      await tester.tap(find.text(l10n.outputWorkspaceEdit));
      await tester.pumpAndSettle();
      expect(find.byType(TextField), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'Edited working copy');
      await tester.pump();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller?.text,
        'Edited working copy',
      );
      final saveAction = find.ancestor(
        of: find.text(l10n.outputWorkspaceSaveVersion),
        matching: find.byType(FilledButton),
      );
      expect(tester.widget<FilledButton>(saveAction).onPressed, isNotNull);
      await tester.tap(saveAction);
      for (var frame = 0; frame < 8; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      final savedOutput =
          await (database.select(
                database.savedOutputs,
              )..where((row) => row.conversationId.equals('conversation-edit')))
              .getSingle();
      final versions = await (database.select(
        database.savedOutputVersions,
      )..where((row) => row.conversationId.equals('conversation-edit'))).get();
      expect(savedOutput.selectedVersion, 2);
      expect(savedOutput.content, 'Edited working copy');
      expect(versions, hasLength(2));
      expect(
        (await repository.getMessages('conversation-edit')).single.content,
        '# Original response\n\n```dart\nfinal value = 1;\n```',
      );
      expect(tester.takeException(), isNull);
      await tester.pump(const Duration(seconds: 5));
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(seconds: 1));
    },
  );

  for (final appearance in [
    (name: 'light', theme: OpenChatTheme.light),
    (name: 'dark', theme: OpenChatTheme.dark),
  ]) {
    testWidgets('matches the ${appearance.name} saved answers page golden', (
      tester,
    ) async {
      await _expectOutputsGolden(
        tester,
        theme: appearance.theme,
        goldenPath: 'goldens/outputs-${appearance.name}.png',
      );
    });
  }
}

Future<void> _expectOutputsGolden(
  WidgetTester tester, {
  required ThemeData theme,
  required String goldenPath,
}) async {
  tester.view.physicalSize = const Size(1280, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);

  final database = OpenChatDatabase(NativeDatabase.memory());
  addTearDown(database.close);
  final repository = ChatRepository(database);
  await repository.createConversation(
    id: 'conversation-output-golden',
    title: 'Product research notes',
    createdAt: DateTime.utc(2026, 10, 9, 12),
  );
  await database
      .into(database.savedOutputs)
      .insert(
        SavedOutputsCompanion.insert(
          conversationId: 'conversation-output-golden',
          messageId: 'answer-output-golden',
          conversationTitle: 'Product research notes',
          content: 'A useful answer can be saved here and revisited from the saved answers page.',
          providerId: const Value<String?>('chatgpt'),
          modelId: const Value<String?>('gpt-5.5'),
          savedAt: DateTime.utc(2026, 10, 9, 12).millisecondsSinceEpoch,
        ),
      );

  await tester.pumpWidget(
    MaterialApp(
      locale: const Locale('tr'),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: theme.copyWith(platform: TargetPlatform.windows),
      home: RepaintBoundary(
        key: const ValueKey<String>('outputs-page-screenshot'),
        child: Scaffold(
          body: OutputsPage(repository: repository, onOpenConversation: (_) {}),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();

  expect(find.text('Product research notes'), findsOneWidget);
  expect(find.textContaining('gpt-5.5'), findsOneWidget);
  await expectLater(
    find.byKey(const ValueKey<String>('outputs-page-screenshot')),
    matchesGoldenFile(goldenPath),
  );
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump(const Duration(seconds: 1));
}

Widget _testApp(Widget child) => MaterialApp(
  locale: const Locale('tr'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: OpenChatTheme.light,
  home: Scaffold(body: child),
);
