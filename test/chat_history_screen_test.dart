import 'package:drift/native.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:zihora/app/zihora_theme.dart';
import 'package:zihora/features/chat/data/chat_repository.dart';
import 'package:zihora/features/chat/data/zihora_database.dart';
import 'package:zihora/features/chat/domain/chat_message.dart';
import 'package:zihora/features/chat/domain/history_storage_status.dart';
import 'package:zihora/features/chat/presentation/chat_screen.dart';
import 'package:zihora/l10n/generated/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    final fontLoader = FontLoader('Manrope')
      ..addFont(rootBundle.load('assets/fonts/Manrope[wght].ttf'));
    await fontLoader.load();
  });

  testWidgets('opens saved chats and reflects local storage in settings', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1680, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final database = ZihoraDatabase(NativeDatabase.memory());
    final repository = ChatRepository(database);

    await repository.createConversation(
      id: 'saved-conversation',
      title: 'Kaydedilmiş sohbet',
      createdAt: DateTime.utc(2026, 9, 26, 12),
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
        theme: ZihoraTheme.light.copyWith(platform: TargetPlatform.windows),
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

    expect(find.text('Eklentiler'), findsOneWidget);
    expect(find.text('Zamanlananlar'), findsOneWidget);
    expect(find.text('Tasarım'), findsOneWidget);
    expect(find.text('Güvenlik'), findsOneWidget);
    expect(find.text('Kaydedilmiş sohbet'), findsOneWidget);

    await tester.tap(find.text('Kaydedilmiş sohbet'));
    await tester.pumpAndSettle();
    expect(find.text('Kaydedilmiş yanıt'), findsOneWidget);

    await tester.tap(find.text('Ayarlar').first);
    await tester.pumpAndSettle();
    expect(find.text('Konuşmalar bu cihazda saklanır.'), findsOneWidget);
    expect(find.text('Bu cihazda'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 1));
    await database.close();
  });
}
