import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/chat/domain/chat_file_change.dart';
import 'package:openchat/features/chat/domain/chat_message.dart';

void main() {
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
