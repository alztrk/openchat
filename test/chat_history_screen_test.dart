import 'package:drift/native.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/presentation/chat_screen.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:shared_preferences_platform_interface/in_memory_shared_preferences_async.dart';
import 'package:shared_preferences_platform_interface/shared_preferences_async_platform_interface.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    SharedPreferencesAsyncPlatform.instance =
        InMemorySharedPreferencesAsync.empty();
    final iconFont = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await iconFont.load();
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
  });

  tearDownAll(() => SharedPreferencesAsyncPlatform.instance = null);

  testWidgets('opens saved chats and reflects local storage in settings', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1680, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final database = OpenChatDatabase(NativeDatabase.memory());
    final repository = ChatRepository(database);

    await repository.createConversation(
      id: 'saved-conversation',
      title: 'Kaydedilmiş sohbet',
      createdAt: DateTime.utc(2026, 9, 26, 12),
      providerId: 'chatgpt',
      connectionId: 'connection-1',
      workspaceId: 'workspace-1',
      modelId: 'model-1',
    );
    await repository.saveMessage(
      conversationId: 'saved-conversation',
      message: const ChatMessage(
        id: 'saved-message',
        role: ChatMessageRole.assistant,
        content: 'Kaydedilmiş yanıt',
      ),
    );

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
        home: ChatScreen(
          themeMode: ThemeMode.light,
          onThemeModeChanged: (_) async {},
          onToggleTheme: () async {},
          chatRepository: repository,
          historyStorageStatus: HistoryStorageStatus.available,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Eklentiler'), findsNothing);
    expect(find.text('Zamanlananlar'), findsNothing);
    expect(find.text('Tasarım'), findsNothing);
    expect(find.text('Güvenlik'), findsNothing);
    expect(find.text('Kaydedilmiş sohbet'), findsOneWidget);

    await tester.tap(find.text('Kaydedilmiş sohbet'));
    await tester.pumpAndSettle();
    expect(find.text('Kaydedilmiş yanıt', findRichText: true), findsOneWidget);

    await tester.tap(find.text('Ayarlar').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Yerel veriler'));
    await tester.pumpAndSettle();
    expect(find.text('Konuşmalar bu cihazda saklanır.'), findsOneWidget);
    expect(find.text('Bu cihazda'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 1));
    await database.close();
  });
}
