import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/features/settings/domain/personal_memory_request_policy.dart';

void main() {
  test('excludes saved facts from the first request in a new chat', () {
    expect(
      shouldIncludePersonalMemoryForRequest(
        isEnabled: true,
        conversationId: 'new-conversation',
        excludedConversationIds: const <String>{},
        excludeNewConversation: true,
      ),
      isFalse,
    );
  });

  test('excludes saved facts from an existing opted-out conversation', () {
    expect(
      shouldIncludePersonalMemoryForRequest(
        isEnabled: true,
        conversationId: 'excluded-conversation',
        excludedConversationIds: const <String>{'excluded-conversation'},
        excludeNewConversation: false,
      ),
      isFalse,
    );
  });

  test(
    'includes saved facts for an enabled conversation without exclusion',
    () {
      expect(
        shouldIncludePersonalMemoryForRequest(
          isEnabled: true,
          conversationId: 'included-conversation',
          excludedConversationIds: const <String>{},
          excludeNewConversation: false,
        ),
        isTrue,
      );
    },
  );

  test('does not include saved facts when the feature is disabled', () {
    expect(
      shouldIncludePersonalMemoryForRequest(
        isEnabled: false,
        conversationId: 'conversation',
        excludedConversationIds: const <String>{},
        excludeNewConversation: false,
      ),
      isFalse,
    );
  });
}
