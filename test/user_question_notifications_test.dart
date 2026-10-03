import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:openchat/platform/windows/user_question_notifications.dart';

void main() {
  test('notification payload accepts only bounded question targets', () {
    final target = UserQuestionNotificationTarget.fromPayload(
      jsonEncode(<String, String>{
        'groupId': 'group-1',
        'conversationId': 'conversation_1',
      }),
    );

    expect(target?.groupId, 'group-1');
    expect(target?.conversationId, 'conversation_1');
    expect(
      UserQuestionNotificationTarget.fromPayload(
        jsonEncode(<String, String>{
          'groupId': 'group/1',
          'conversationId': 'conversation-1',
        }),
      ),
      isNull,
    );
    expect(
      UserQuestionNotificationTarget.fromPayload(List.filled(1025, 'x').join()),
      isNull,
    );
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
