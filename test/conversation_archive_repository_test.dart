import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/settings/data/conversation_archive_repository.dart';

void main() {
  group('ConversationArchiveSummary', () {
    test('parses the pre-restore archive inspection response', () {
      final summary = ConversationArchiveSummary.fromJson(<String, Object?>{
        'formatVersion': 1,
        'conversationCount': 3,
        'messageCount': 28,
        'attachmentCount': 2,
        'attachmentBytes': 4096,
        'duplicateConversationCount': 1,
        'createdAtUnixMs': 1_800_000_000_000,
      });

      expect(summary.formatVersion, 1);
      expect(summary.conversationCount, 3);
      expect(summary.messageCount, 28);
      expect(summary.attachmentCount, 2);
      expect(summary.attachmentBytes, 4096);
      expect(summary.duplicateConversationCount, 1);
      expect(summary.createdAtUnixMs, 1_800_000_000_000);
    });

    test('rejects missing or negative archive counts', () {
      expect(
        () => ConversationArchiveSummary.fromJson(<String, Object?>{
          'formatVersion': 1,
          'conversationCount': -1,
          'messageCount': 0,
          'attachmentCount': 0,
          'attachmentBytes': 0,
          'duplicateConversationCount': 0,
          'createdAtUnixMs': 0,
        }),
        throwsFormatException,
      );
      expect(
        () => ConversationArchiveSummary.fromJson(<String, Object?>{}),
        throwsFormatException,
      );
    });
  });

  group('ConversationArchiveResult', () {
    test('accepts export and restore response shapes', () {
      final exported = ConversationArchiveResult.fromJson(<String, Object?>{
        'conversationCount': 2,
        'messageCount': 10,
        'attachmentCount': 1,
        'attachmentBytes': 256,
      });
      expect(exported.conversationCount, 2);
      expect(exported.messageCount, 10);
      expect(exported.attachmentCount, 1);
      expect(exported.attachmentBytes, 256);
      expect(exported.duplicateConversationCount, 0);

      final restored = ConversationArchiveResult.fromJson(<String, Object?>{
        'restoredConversationCount': 1,
        'restoredMessageCount': 4,
        'restoredAttachmentCount': 0,
        'duplicateConversationCount': 1,
      });
      expect(restored.conversationCount, 1);
      expect(restored.messageCount, 4);
      expect(restored.attachmentCount, 0);
      expect(restored.duplicateConversationCount, 1);
    });

    test('maps conflict policies to the stable service contract', () {
      expect(
        ConversationArchiveConflictPolicy.skipExisting.rpcValue,
        'skip_existing',
      );
      expect(
        ConversationArchiveConflictPolicy.importAsCopy.rpcValue,
        'import_as_copy',
      );
    });
  });
}
