import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/features/chat/data/chat_file_changes_repository.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';
import 'package:openchat/features/chat/presentation/widgets/chat_file_changes_panel.dart';
import 'package:openchat/l10n/generated/app_localizations.dart';
import 'package:openchat/platform/windows/openchat_service_client.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'parses file change summaries and preserves their serialized contract',
    () {
      final change = ChatFileChange.fromJson(_changeJson());

      expect(change.path, 'lib/example.dart');
      expect(change.kind, ChatFileChangeKind.modified);
      expect(change.status, ChatFileChangeState.active);
      expect(change.addedLines, 3);
      expect(change.removedLines, 1);
      expect(ChatFileChange.fromJson(change.toJson()).id, change.id);
    },
  );

  test('rejects malformed file change summaries and diffs', () {
    final invalid = _changeJson()..['canRevert'] = 'yes';
    expect(() => ChatFileChange.fromJson(invalid), throwsFormatException);
    expect(
      () => ChatFileChangeDiff.fromJson(<String, Object?>{
        'path': 'lib/example.dart',
        'isBinary': false,
        'available': true,
        'diff': 42,
      }),
      throwsFormatException,
    );
  });

  test('loads tool history written before file change tracking existed', () {
    final activity = ChatToolActivity.fromJson(<String, Object?>{
      'callId': 'call-1',
      'name': 'read_file',
      'arguments': <String, Object?>{'path': 'lib/example.dart'},
      'output': 'contents',
      'status': 'completed',
    });

    expect(activity.fileChanges, isEmpty);
    expect(activity.fileChangesError, isNull);
  });

  test('parses and validates a file diff response', () {
    final diff = ChatFileChangeDiff.fromJson(<String, Object?>{
      'path': 'lib/example.dart',
      'isBinary': false,
      'available': true,
      'truncated': true,
      'diff': '+new line',
    });

    expect(diff.diff, '+new line');
    expect(diff.isTruncated, isTrue);
  });

  testWidgets('shows a file diff and marks the change after reverting it', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 760);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final service = _FakeFileChangesServiceClient();
    List<ChatFileChange> updatedChanges = const <ChatFileChange>[];

    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('tr'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenChatTheme.dark.copyWith(platform: TargetPlatform.windows),
        home: Scaffold(
          body: SizedBox(
            width: 900,
            height: 700,
            child: ChatFileChangesPanel(
              repository: ChatFileChangesRepository(service),
              conversationId: 'conversation-1',
              onClose: () {},
              onChangesUpdated: (changes) => updatedChanges = changes,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('lib/example.dart'), findsNWidgets(2));
    expect(find.textContaining('+new line'), findsOneWidget);
    expect(find.text('Geri al'), findsOneWidget);

    await tester.tap(find.text('Geri al'));
    await tester.pumpAndSettle();

    expect(service.revertCount, 1);
    expect(find.text('Geri alındı'), findsOneWidget);
    expect(find.text('Geri al'), findsNothing);
    expect(updatedChanges.single.status, ChatFileChangeState.reverted);
    expect(
      service.calls,
      containsAll(<String>[
        'chat.file_changes.list',
        'chat.file_changes.diff',
        'chat.file_changes.revert',
      ]),
    );
  });
}

Map<String, Object?> _changeJson() => <String, Object?>{
  'id': 'a' * 64,
  'path': 'lib/example.dart',
  'kind': 'modified',
  'status': 'active',
  'addedLines': 3,
  'removedLines': 1,
  'isBinary': false,
  'diffAvailable': true,
  'canRevert': true,
};

class _FakeFileChangesServiceClient extends OpenChatServiceClient {
  bool reverted = false;
  int revertCount = 0;
  final List<String> calls = <String>[];

  @override
  Future<Map<String, Object?>> call(
    String method, {
    Map<String, Object?> params = const <String, Object?>{},
    Duration timeout = const Duration(seconds: 15),
  }) async {
    calls.add(method);
    return switch (method) {
      'chat.file_changes.list' || 'chat.file_changes.revert' => _listResponse(),
      'chat.file_changes.diff' => <String, Object?>{
        'path': 'lib/example.dart',
        'isBinary': false,
        'available': true,
        'truncated': false,
        'diff': '--- a/lib/example.dart\n+++ b/lib/example.dart\n+new line',
      },
      _ => throw StateError('Unexpected service method: $method'),
    };
  }

  Map<String, Object?> _listResponse() {
    if (calls.last == 'chat.file_changes.revert') {
      reverted = true;
      revertCount += 1;
    }
    return <String, Object?>{
      'changes': <Map<String, Object?>>[
        <String, Object?>{
          'id': 'change-1',
          'path': 'lib/example.dart',
          'kind': 'modified',
          'status': reverted ? 'reverted' : 'active',
          'addedLines': 1,
          'removedLines': 0,
          'isBinary': false,
          'diffAvailable': true,
          'canRevert': !reverted,
        },
      ],
    };
  }
}
