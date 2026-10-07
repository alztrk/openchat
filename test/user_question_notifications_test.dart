import 'dart:convert';
import 'dart:ui' show AppLifecycleState;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/platform/windows/user_question_notifications.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('notification payload accepts only bounded question targets', () {
    final target = ConversationNotificationTarget.fromPayload(
      jsonEncode(<String, String>{
        'groupId': 'group-1',
        'conversationId': 'conversation_1',
      }),
    );

    expect(target?.groupId, 'group-1');
    expect(target?.kind, ConversationNotificationKind.userQuestion);
    expect(target?.conversationId, 'conversation_1');
    expect(
      ConversationNotificationTarget.fromPayload(
        jsonEncode(<String, String>{
          'groupId': 'group/1',
          'conversationId': 'conversation-1',
        }),
      ),
      isNull,
    );
    expect(
      ConversationNotificationTarget.fromPayload(List.filled(1025, 'x').join()),
      isNull,
    );
  });

  test(
    'assistant response payload opens its conversation without a question',
    () {
      final target = ConversationNotificationTarget.fromPayload(
        jsonEncode(<String, String>{
          'type': 'assistant_response',
          'conversationId': 'conversation_1',
        }),
      );

      expect(target?.kind, ConversationNotificationKind.assistantResponse);
      expect(target?.conversationId, 'conversation_1');
      expect(target?.groupId, isNull);
      expect(
        ConversationNotificationTarget.fromPayload(
          jsonEncode(<String, Object?>{
            'type': 'assistant_response',
            'conversationId': 'conversation_1',
            'groupId': 'group-1',
          }),
        ),
        isNull,
      );
    },
  );

  test('assistant response notification captures a bounded quick reply', () {
    final payload = jsonEncode(<String, String>{
      'type': 'assistant_response_reply',
      'conversationId': 'conversation_1',
      'assistantMessageId': 'assistant-message-1',
    });
    final target = ConversationNotificationTarget.fromNotificationResponse(
      NotificationResponse(
        notificationResponseType:
            NotificationResponseType.selectedNotificationAction,
        payload: payload,
        data: const <String, Object?>{
          'assistant_reply': '  Continue, please.  ',
        },
      ),
    );

    expect(target?.kind, ConversationNotificationKind.assistantResponseReply);
    expect(target?.conversationId, 'conversation_1');
    expect(target?.assistantMessageId, 'assistant-message-1');
    expect(target?.replyText, 'Continue, please.');
    expect(target?.replyInputTooLong, isFalse);

    final oversizedTarget =
        ConversationNotificationTarget.fromNotificationResponse(
          NotificationResponse(
            notificationResponseType:
                NotificationResponseType.selectedNotificationAction,
            payload: payload,
            data: <String, Object?>{
              'assistant_reply': List<String>.filled(4097, '😊').join(),
            },
          ),
        );
    expect(oversizedTarget?.replyInputTooLong, isTrue);
    expect(oversizedTarget?.replyText, isNull);
  });

  test('assistant response quick reply requires a safe message identifier', () {
    expect(
      ConversationNotificationTarget.fromPayload(
        jsonEncode(<String, String>{
          'type': 'assistant_response_reply',
          'conversationId': 'conversation_1',
          'assistantMessageId': 'message/1',
        }),
      ),
      isNull,
    );
  });

  test('assistant response preview is a normalized 160-character excerpt', () {
    expect(
      UserQuestionNotifications.assistantResponsePreview(
        '  First line\n\nsecond line.  ',
      ),
      'First line second line.',
    );
    final preview = UserQuestionNotifications.assistantResponsePreview(
      List<String>.filled(170, '😊').join(),
    );
    expect(preview.runes.length, 160);
    expect(preview.endsWith('…'), isTrue);
  });

  test('assistant response notifications require a non-resumed lifecycle', () {
    expect(
      UserQuestionNotifications.isApplicationBackgrounded(
        AppLifecycleState.resumed,
      ),
      isFalse,
    );
    for (final lifecycleState in <AppLifecycleState>[
      AppLifecycleState.inactive,
      AppLifecycleState.hidden,
      AppLifecycleState.paused,
      AppLifecycleState.detached,
    ]) {
      expect(
        UserQuestionNotifications.isApplicationBackgrounded(lifecycleState),
        isTrue,
      );
    }
    expect(UserQuestionNotifications.isApplicationBackgrounded(null), isTrue);
  });

  test('assistant response notifications map bundled provider icons', () async {
    const expectedAssets = <String, String>{
      'chatgpt': 'assets/icons/notification_chatgpt.svg',
      'opencode': 'assets/icons/notification_opencode.svg',
      'gemini': 'assets/icons/notification_sparkles.svg',
      'groq': 'assets/icons/notification_zap.svg',
      'cerebras': 'assets/icons/notification_microchip.svg',
      'openrouter': 'assets/icons/notification_openrouter.svg',
      'mistral': 'assets/icons/mistral.png',
      'llama_cpp': 'assets/icons/notification_llama_cpp.svg',
      'vllm': 'assets/icons/notification_vllm.svg',
      'exllama': 'assets/icons/engines/exllama-v3.png',
    };
    for (final entry in expectedAssets.entries) {
      final iconAsset = UserQuestionNotifications.providerIconAssetFor(
        entry.key,
      );
      expect(iconAsset, entry.value);
      await rootBundle.load(iconAsset);
    }
  });

  test(
    'notification initialization skips non-Windows UI test targets',
    () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.android;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      final notifications = UserQuestionNotifications(
        plugin: FlutterLocalNotificationsPlugin(),
      );

      final launchTarget = await notifications.initialize(
        onSelected: (_) => fail('No notification should be selected.'),
      );
      await notifications.show(
        groupId: 'group-1',
        conversationId: 'conversation-1',
        title: 'Waiting',
        body: 'The AI is waiting.',
      );

      expect(launchTarget, isNull);
    },
  );
}
