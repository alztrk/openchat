import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/conversation_memory_repository.dart';
import 'package:openchat/features/chat/domain/conversation_memory.dart';
import 'package:openchat/features/chat/presentation/widgets/conversation_memory_dialog.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('shows compacted context and searches scoped archive results', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();

    await tester.pumpWidget(_testApp(service));
    await tester.pumpAndSettle();

    expect(find.text('Older preferences summary.'), findsOneWidget);
    expect(find.textContaining('128 girdi tokeni'), findsOneWidget);
    expect(find.textContaining('OpenCode · test-model'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'Atlas archive');
    await tester.tap(find.text('Ara'));
    await tester.pumpAndSettle();

    expect(find.text('Atlas archive result.'), findsOneWidget);
    expect(find.textContaining('Kullanıcı mesajı'), findsOneWidget);
    expect(
      service.calls,
      contains('chat.memory.search:conversation-1:Atlas archive'),
    );
  });

  testWidgets('requires confirmation before resetting compacted context', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();

    await tester.pumpWidget(_testApp(service));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Bağlamı sıfırla'));
    await tester.pumpAndSettle();

    expect(
      find.textContaining('Tam mesaj geçmişi ve arşiv korunur'),
      findsOneWidget,
    );
    await tester.tap(find.text('Vazgeç'));
    await tester.pumpAndSettle();
    expect(service.resetCount, 0);

    await tester.tap(find.text('Bağlamı sıfırla'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Sıfırla'));
    await tester.pumpAndSettle();

    expect(service.resetCount, 1);
    expect(find.text('Henüz sıkıştırılmış bir özet yok.'), findsOneWidget);
  });

  testWidgets('rejects short archive queries without calling the service', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();

    await tester.pumpWidget(_testApp(service));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'a');
    await tester.tap(find.text('Ara'));
    await tester.pumpAndSettle();

    expect(find.text('Arama için en az iki karakter yaz.'), findsOneWidget);
    expect(
      service.calls.any((call) => call.startsWith('chat.memory.search:')),
      isFalse,
    );
  });

  testWidgets(
    'lets users exclude conversations and individual tools from archive indexing',
    (tester) async {
      _setViewport(tester);
      final service = _FakeMemoryServiceClient();

      await tester.pumpWidget(_testApp(service));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Arşiv indeksleme'));
      await tester.pumpAndSettle();

      expect(find.text('Bu sohbeti dahil et'), findsOneWidget);
      expect(find.text('read'), findsOneWidget);

      final conversationSwitch = find.descendant(
        of: find.widgetWithText(SwitchListTile, 'Bu sohbeti dahil et'),
        matching: find.byType(Switch),
      );
      await tester.tap(conversationSwitch);
      await tester.pumpAndSettle();
      expect(
        service.calls,
        contains('chat.memory.archive.set_conversation:conversation-1:false'),
      );
      expect(
        find.text(
          'Bu sohbetin türetilmiş arşiv indeksleri silinir ve yeni indekslemeye alınmaz.',
        ),
        findsNWidgets(2),
      );

      await tester.tap(conversationSwitch);
      await tester.pumpAndSettle();
      final toolSwitch = find.descendant(
        of: find.widgetWithText(SwitchListTile, 'read'),
        matching: find.byType(Switch),
      );
      await tester.ensureVisible(toolSwitch);
      await tester.pumpAndSettle();
      await tester.tap(toolSwitch);
      await tester.pumpAndSettle();
      expect(
        service.calls,
        contains('chat.memory.archive.set_tool:conversation-1:read:false'),
      );
      expect(
        find.text(
          'Bu aracın kayıtlı ayrıntıları arşiv indekslerinden silinir.',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('downloads the semantic model only after the user requests it', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();
    final repository = _FakeConversationMemoryRepository(service);
    addTearDown(repository.closeProgress);

    await tester.pumpWidget(_testApp(service, repository: repository));
    await tester.pumpAndSettle();

    expect(find.text('Hazırla'), findsOneWidget);
    expect(
      find.text('Hazırlamadan önce anahtar kelime araması kullanılabilir.'),
      findsOneWidget,
    );
    expect(repository.prepareCount, 0);

    await tester.tap(find.text('Hazırla'));
    await tester.pumpAndSettle();

    expect(repository.prepareCount, 1);
    expect(find.text('Yerel anlamsal arama hazır'), findsOneWidget);
  });

  testWidgets('shows semantic download progress and can resume after cancel', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();
    final repository = _FakeConversationMemoryRepository(
      service,
      holdPreparation: true,
    );
    addTearDown(repository.closeProgress);

    await tester.pumpWidget(_testApp(service, repository: repository));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Hazırla'));
    await tester.pump();

    expect(find.text('Vazgeç'), findsOneWidget);
    repository.progress.add(
      const SemanticPreparationProgress(
        phase: SemanticPreparationPhase.downloading,
        downloadedBytes: 50000000,
        totalBytes: 135400000,
      ),
    );
    await tester.pump();
    expect(find.textContaining('%37 indirildi'), findsOneWidget);

    await tester.tap(find.text('Vazgeç'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(repository.cancelCount, 1);
    expect(
      find.text('İndirme iptal edildi. Anahtar kelime araması kullanılabilir.'),
      findsOneWidget,
    );
  });

  testWidgets('keeps context reset disabled while a response is running', (
    tester,
  ) async {
    _setViewport(tester);
    final service = _FakeMemoryServiceClient();

    await tester.pumpWidget(_testApp(service, isSending: true));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Bağlamı sıfırla'));
    await tester.pumpAndSettle();

    expect(find.text('Sıkıştırılmış bağlam sıfırlansın mı?'), findsNothing);
    expect(service.resetCount, 0);
  });
}

void _setViewport(WidgetTester tester) {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 900);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

Widget _testApp(
  OpenChatServiceClient service, {
  bool isSending = false,
  ConversationMemoryRepository? repository,
}) => MaterialApp(
  locale: const Locale('tr'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
  home: Scaffold(
    body: Center(
      child: ConversationMemoryDialog(
        repository: repository ?? ConversationMemoryRepository(service),
        conversationId: 'conversation-1',
        isSending: isSending,
      ),
    ),
  ),
);

class _FakeMemoryServiceClient extends OpenChatServiceClient {
  final List<String> calls = <String>[];
  String? summary = 'Older preferences summary.';
  int resetCount = 0;
  bool semanticReady = false;
  bool archiveIncluded = true;
  bool readToolIncluded = true;

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    if (method == 'chat.memory.inspect') {
      calls.add('$method:${params['conversationId']}');
      final currentSummary = summary;
      return <String, Object?>{
        'compactionKind': currentSummary == null ? null : 'summary',
        'summary': currentSummary,
        'lastPrompt': currentSummary == null
            ? null
            : <String, Object?>{
                'inputTokens': 128,
                'providerId': 'opencode',
                'modelId': 'test-model',
              },
        'archiveIndexSettings': _archiveSettings(),
      };
    }
    if (method == 'chat.memory.archive.set_conversation') {
      final included = params['included'];
      if (included is! bool) throw StateError('Missing archive selection.');
      archiveIncluded = included;
      calls.add('$method:${params['conversationId']}:$archiveIncluded');
      return <String, Object?>{'archiveIndexSettings': _archiveSettings()};
    }
    if (method == 'chat.memory.archive.set_tool') {
      final included = params['included'];
      if (included is! bool) throw StateError('Missing tool selection.');
      readToolIncluded = included;
      calls.add(
        '$method:${params['conversationId']}:${params['toolName']}:$readToolIncluded',
      );
      return <String, Object?>{'archiveIndexSettings': _archiveSettings()};
    }
    if (method == 'chat.memory.semantic.status') {
      calls.add(method);
      return <String, Object?>{'ready': semanticReady};
    }
    if (method == 'chat.memory.semantic.prepare') {
      calls.add(method);
      semanticReady = true;
      return <String, Object?>{'ready': true};
    }
    if (method == 'chat.memory.search') {
      calls.add('$method:${params['conversationId']}:${params['query']}');
      return <String, Object?>{
        'results': <Map<String, Object?>>[
          <String, Object?>{
            'messageId': 'message-archive-1',
            'role': 'user',
            'content': 'Atlas [match]archive[/match] result.',
            'createdAtUnixMs': 1750000000000,
          },
        ],
      };
    }
    if (method == 'chat.memory.reset_compaction') {
      calls.add('$method:${params['conversationId']}');
      resetCount++;
      summary = null;
      return <String, Object?>{'reset': true};
    }
    throw StateError('Unexpected method $method.');
  }

  Map<String, Object?> _archiveSettings() => <String, Object?>{
    'included': archiveIncluded,
    'tools': <Map<String, Object?>>[
      <String, Object?>{'name': 'read', 'included': readToolIncluded},
    ],
  };
}

class _FakeConversationMemoryRepository extends ConversationMemoryRepository {
  _FakeConversationMemoryRepository(
    super.service, {
    this.holdPreparation = false,
  });

  final bool holdPreparation;
  final StreamController<SemanticPreparationProgress> progress =
      StreamController<SemanticPreparationProgress>();
  final Completer<bool> _preparationResult = Completer<bool>();
  int prepareCount = 0;
  int cancelCount = 0;

  @override
  Future<SemanticSearchPreparation> prepareSemanticSearch() async {
    prepareCount++;
    if (!holdPreparation) _preparationResult.complete(true);
    return SemanticSearchPreparation(
      progress: progress.stream,
      completed: _preparationResult.future,
      cancel: () async {
        cancelCount++;
        if (!_preparationResult.isCompleted) _preparationResult.complete(false);
        return true;
      },
    );
  }

  Future<void> closeProgress() => progress.close();
}
