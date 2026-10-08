import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/settings/domain/usage_statistics.dart';

void main() {
  test('parses optional durable run attribution on a request record', () {
    final request = UsageStatisticsRequest.fromJson(<String, Object?>{
      'eventId': 'request-1',
      'eventKind': 'request',
      'conversationId': 'conversation-1',
      'conversationTitle': 'Subagent review',
      'operation': 'tool_follow_up',
      'status': 'completed',
      'usageSource': 'provider_reported',
      'startedAtUnixMs': 1,
      'runId': 'child-run-1',
    });

    expect(request.runId, 'child-run-1');
    expect(
      UsageStatisticsRequest.fromJson(<String, Object?>{
        'eventId': 'request-2',
        'eventKind': 'request',
        'conversationId': 'conversation-1',
        'conversationTitle': 'Legacy request',
        'operation': 'chat',
        'status': 'completed',
        'usageSource': 'unavailable',
        'startedAtUnixMs': 2,
      }).runId,
      isNull,
    );
  });

  test('parses the privacy-preserving request manifest', () {
    final manifest = UsageRequestManifest.fromJson(<String, Object?>{
      'available': true,
      'messageCount': 3,
      'messageRoles': <String, Object?>{'system': 1, 'user': 1, 'assistant': 1},
      'imageCount': 1,
      'toolResultCount': 2,
      'instructionBytes': 480,
      'instructionSources': <String>['shared', 'project'],
      'toolDefinitions': <String>['read', 'search'],
      'cacheControls': <String>['cache_control'],
      'sourceMessageIds': <String>['message-1', 'message-2'],
      'archivedMessageIds': <String>['message-old'],
      'summarizedThroughMessageId': 'message-boundary',
      'sourceAttachments': <Object?>[
        <String, Object?>{
          'id': 'attachment-1',
          'messageId': 'message-2',
          'name': 'notes.txt',
          'kind': 'text',
          'mimeType': 'text/plain',
        },
      ],
      'sourceLimits': <String, Object?>{
        'messageIdsTruncated': false,
        'archivedMessageIdsTruncated': false,
        'attachmentsTruncated': false,
      },
    });

    expect(manifest.available, isTrue);
    expect(manifest.messageCount, 3);
    expect(manifest.messageRoles, <String, int>{
      'system': 1,
      'user': 1,
      'assistant': 1,
    });
    expect(manifest.imageCount, 1);
    expect(manifest.toolResultCount, 2);
    expect(manifest.instructionBytes, 480);
    expect(manifest.instructionSources, <String>['shared', 'project']);
    expect(manifest.toolDefinitions, <String>['read', 'search']);
    expect(manifest.cacheControls, <String>['cache_control']);
    expect(manifest.sourceMessageIds, <String>['message-1', 'message-2']);
    expect(manifest.archivedMessageIds, <String>['message-old']);
    expect(manifest.summarizedThroughMessageId, 'message-boundary');
    expect(manifest.sourceAttachments.single.name, 'notes.txt');
    expect(manifest.sourceDetailsTruncated, isFalse);
  });

  test('accepts request manifests saved before source details were added', () {
    final manifest = UsageRequestManifest.fromJson(<String, Object?>{
      'available': true,
      'messageCount': 1,
      'messageRoles': <String, Object?>{'user': 1},
      'imageCount': 0,
      'toolResultCount': 0,
      'instructionBytes': 0,
      'instructionSources': <String>[],
      'toolDefinitions': <String>[],
      'cacheControls': <String>[],
    });

    expect(manifest.sourceMessageIds, isEmpty);
    expect(manifest.sourceAttachments, isEmpty);
    expect(manifest.sourceDetailsTruncated, isFalse);
  });

  test('rejects negative manifest counts', () {
    expect(
      () => UsageRequestManifest.fromJson(<String, Object?>{
        'available': true,
        'messageCount': -1,
        'messageRoles': <String, Object?>{},
        'imageCount': 0,
        'toolResultCount': 0,
        'instructionBytes': 0,
        'instructionSources': <String>[],
        'toolDefinitions': <String>[],
        'cacheControls': <String>[],
      }),
      throwsFormatException,
    );
  });
}
