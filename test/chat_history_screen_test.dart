import 'dart:async';

import 'package:drift/native.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/data/openchat_database.dart';
import 'package:openchat/features/chat/domain/default_model_preference.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/domain/history_storage_status.dart';
import 'package:openchat/features/chat/presentation/chat_screen.dart';
import 'package:openchat/features/chat/presentation/widgets/model_selector.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_composer.dart';
import 'package:openchat/features/settings/data/open_code_api_key_store.dart';
import 'package:openchat/features/settings/data/settings_preferences.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

import 'support/shad_test_scope.dart';

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';
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
    final uiFontLoader = FontLoader(OpenChatTypography.uiFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceSans3VF-Upright.ttf'));
    await uiFontLoader.load();
    final codeFontLoader = FontLoader(OpenChatTypography.codeFontFamily)
      ..addFont(rootBundle.load('assets/fonts/SourceCodeVF-Upright.ttf'));
    await codeFontLoader.load();
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
        builder: openChatShadTestBuilder,
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

    expect(find.bySemanticsLabel('1. Bağlantılar'), findsNothing);
    expect(find.text('Eklentiler'), findsNothing);
    expect(find.text('Zamanlananlar'), findsNothing);
    expect(find.text('Tasarım'), findsNothing);
    expect(find.text('Güvenlik'), findsNothing);
    expect(find.text('Kaydedilmiş sohbet'), findsOneWidget);

    await tester.tap(find.text('Kaydedilmiş sohbet'));
    await tester.pumpAndSettle();
    expect(find.text('Kaydedilmiş yanıt', findRichText: true), findsOneWidget);

    await tester.tap(find.byTooltip('Ayarlar'));
    await tester.pumpAndSettle();
    expect(find.text('Veri ve gizlilik'), findsOneWidget);
    await tester.ensureVisible(find.byTooltip('Yerel veriler'));
    await tester.tap(find.byTooltip('Yerel veriler'));
    await tester.pumpAndSettle();
    expect(find.text('Konuşmalar bu cihazda saklanır.'), findsOneWidget);
    expect(find.text('Bu cihazda'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 1));
    await database.close();
  });

  testWidgets('shows first-run guidance when no assistant response exists', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final database = OpenChatDatabase(NativeDatabase.memory());
    final repository = ChatRepository(database);

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
        builder: openChatShadTestBuilder,
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

    final firstStep = find.bySemanticsLabel('1. Connections');
    expect(firstStep, findsOneWidget);
    expect(
      tester.getSemantics(firstStep).getSemanticsData().value,
      AppLocalizations.of(tester.element(find.byType(ChatScreen)))!
          .setupStepCurrent,
    );

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 1));
    await database.close();
  });

  testWidgets(
    'keeps a newly selected conversation when earlier send preparation finishes',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      SharedPreferencesAsyncPlatform.instance =
          InMemorySharedPreferencesAsync.empty();

      final preferences = SettingsPreferences(SharedPreferencesAsync());
      await preferences.writeDefaultModel(
        const DefaultModelPreference(
          providerId: 'opencode',
          modelId: 'test-model',
          displayName: 'Test model',
        ),
      );
      final keyStore = _ControlledOpenCodeKeyStore();
      final service = _TemporaryChatTestService();
      final database = OpenChatDatabase(NativeDatabase.memory());
      final repository = ChatRepository(database);
      await repository.createConversation(
        id: 'other-conversation',
        title: 'Other chat',
        createdAt: DateTime.utc(2026, 10, 14),
        providerId: 'opencode',
        modelId: 'test-model',
      );
      await repository.saveMessage(
        conversationId: 'other-conversation',
        message: const ChatMessage(
          id: 'other-message',
          role: ChatMessageRole.assistant,
          content: 'Conversation that should stay selected',
        ),
      );

      Future<void> pumpUi() async {
        for (var frame = 0; frame < 8; frame++) {
          await tester.pump(const Duration(milliseconds: 100));
        }
      }

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
          builder: openChatShadTestBuilder,
          home: ChatScreen(
            themeMode: ThemeMode.light,
            onThemeModeChanged: (_) async {},
            onToggleTheme: () async {},
            chatRepository: repository,
            settingsPreferences: preferences,
            openCodeApiKeyStore: keyStore,
            serviceClient: service,
            historyStorageStatus: HistoryStorageStatus.available,
          ),
        ),
      );
      await pumpUi();
      expect(find.text('Test model'), findsWidgets);
      expect(
        tester.widget<ChatComposer>(find.byType(ChatComposer)).canSendMessage,
        isTrue,
      );

      await tester.enterText(
        find
            .descendant(
              of: find.byType(ChatComposer),
              matching: find.byType(EditableText),
            )
            .first,
        'A request started in a different conversation',
      );
      expect(
        tester.widget<ChatComposer>(find.byType(ChatComposer)).controller.text,
        'A request started in a different conversation',
      );
      expect(find.byTooltip('Send'), findsOneWidget);
      keyStore.delayNextRead();
      tester.widget<ChatComposer>(find.byType(ChatComposer)).onSendMessage();
      await pumpUi();
      expect(keyStore.delayedReadStarted.isCompleted, isTrue);

      await tester.tap(find.text('Other chat'));
      await pumpUi();
      expect(
        find.text('Conversation that should stay selected', findRichText: true),
        findsOneWidget,
      );

      keyStore.completeDelayedRead();
      await service.sendOperationStarted.future;
      await pumpUi();
      expect(
        find.text('Conversation that should stay selected', findRichText: true),
        findsOneWidget,
      );
      await tester.pump(const Duration(seconds: 5));

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(milliseconds: 1));
      await database.close();
    },
  );

  testWidgets(
    'temporary send preparation prevents duplicate sends and session exit',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      SharedPreferencesAsyncPlatform.instance =
          InMemorySharedPreferencesAsync.empty();

      final preferences = SettingsPreferences(SharedPreferencesAsync());
      await preferences.writeDefaultModel(
        const DefaultModelPreference(
          providerId: 'opencode',
          modelId: 'test-model',
          displayName: 'Test model',
        ),
      );
      final keyStore = _ControlledOpenCodeKeyStore();
      final service = _TemporaryChatTestService();
      final database = OpenChatDatabase(NativeDatabase.memory());
      final repository = ChatRepository(database);
      Future<void> pumpUi() async {
        for (var frame = 0; frame < 8; frame++) {
          await tester.pump(const Duration(milliseconds: 100));
        }
      }

      await tester.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenChatTheme.light.copyWith(platform: TargetPlatform.windows),
          builder: openChatShadTestBuilder,
          home: ChatScreen(
            themeMode: ThemeMode.light,
            onThemeModeChanged: (_) async {},
            onToggleTheme: () async {},
            chatRepository: repository,
            settingsPreferences: preferences,
            openCodeApiKeyStore: keyStore,
            serviceClient: service,
            historyStorageStatus: HistoryStorageStatus.available,
          ),
        ),
      );
      await pumpUi();

      await tester.tap(find.byTooltip('Temporary chat'));
      await pumpUi();
      expect(find.text('Temporary chat'), findsOneWidget);
      expect(service.calls, contains('opencode.models.list'));
      expect(find.text('Test model'), findsWidgets);
      final modelSelector = tester.widget<ModelSelector>(
        find.byType(ModelSelector).first,
      );
      expect(modelSelector.selectedModelId, 'test-model');
      expect(
        tester.widget<ChatComposer>(find.byType(ChatComposer)).canSendMessage,
        isTrue,
      );
      await tester.enterText(
        find.byType(EditableText).first,
        'Keep this session',
      );
      await tester.pump();
      expect(
        tester
            .widget<EditableText>(find.byType(EditableText).first)
            .controller
            .text,
        'Keep this session',
      );
      keyStore.delayNextRead();
      await tester.tap(find.byTooltip('Send'));
      await pumpUi();
      expect(keyStore.delayedReadStarted.isCompleted, isTrue);

      await tester.tap(find.text('End'));
      await pumpUi();
      expect(find.text('Temporary chat'), findsOneWidget);
      expect(
        find.text('Stop the response before leaving this temporary chat.'),
        findsOneWidget,
      );

      await tester.tap(find.byTooltip('Send'), warnIfMissed: false);
      await tester.pump();
      expect(keyStore.delayedReadCount, 1);
      expect(service.temporarySendCount, 0);

      keyStore.completeDelayedRead();
      await pumpUi();
      expect(find.textContaining('Send this message to'), findsOneWidget);

      await tester.tap(find.text('Continue'));
      await pumpUi();
      expect(service.temporarySendCount, 1);
      await tester.pump(const Duration(seconds: 5));

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(milliseconds: 1));
      await database.close();
    },
  );
}

class _ControlledOpenCodeKeyStore extends OpenCodeApiKeyStore {
  _ControlledOpenCodeKeyStore() : super(const FlutterSecureStorage());

  final Completer<void> delayedReadStarted = Completer<void>();
  Completer<String?>? _delayedRead;
  int delayedReadCount = 0;

  void delayNextRead() => _delayedRead = Completer<String?>();

  void completeDelayedRead() {
    final delayedRead = _delayedRead;
    _delayedRead = null;
    delayedRead?.complete('test-only-opencode-key');
  }

  @override
  Future<String?> readApiKey() {
    final delayedRead = _delayedRead;
    if (delayedRead == null) {
      return Future<String?>.value('test-only-opencode-key');
    }
    delayedReadCount++;
    if (!delayedReadStarted.isCompleted) delayedReadStarted.complete();
    return delayedRead.future;
  }
}

class _TemporaryChatTestService extends OpenChatServiceClient {
  int temporarySendCount = 0;
  final List<String> calls = [];
  final Completer<void> sendOperationStarted = Completer<void>();

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    if (method == 'chatgpt.connections.list') {
      return <String, Object?>{'connections': <Object?>[]};
    }
    if (method == 'opencode.models.list') {
      return <String, Object?>{
        'freshness': 'current',
        'models': <Object?>[
          <String, Object?>{
            'id': 'test-model',
            'displayName': 'Test model',
            'isAvailable': true,
            'reasoningLevels': <String>[],
            'supportsTools': false,
          },
        ],
      };
    }
    if (method == 'chat.context.usage.estimate') {
      return <String, Object?>{
        'instructionsTokens': 0,
        'toolDefinitions': <Object?>[],
      };
    }
    if (method == 'chat.questions.list') {
      return <String, Object?>{'questions': <Object?>[]};
    }
    if (method == 'chat.goal.active') {
      return <String, Object?>{'goal': null};
    }
    if (method == 'chat.file_changes.list') {
      return <String, Object?>{'changes': <Object?>[]};
    }
    if (method == 'chat.memory.inspect') {
      return <String, Object?>{
        'compactionKind': null,
        'archiveIndexSettings': <String, Object?>{
          'included': false,
          'tools': <Object?>[],
        },
      };
    }
    return <String, Object?>{};
  }

  @override
  Future<OpenChatServiceOperation> startOperation(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
  }) async {
    if (method == 'chat.temporary.send') temporarySendCount++;
    if (method == 'chat.send' && !sendOperationStarted.isCompleted) {
      sendOperationStarted.complete();
    }
    throw const OpenChatServiceException(
      code: 'service_unavailable',
      message: 'The local service is unavailable.',
    );
  }
}
