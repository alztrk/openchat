import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

class UserQuestionNotificationTarget {
  const UserQuestionNotificationTarget({
    required this.groupId,
    required this.conversationId,
  });

  final String groupId;
  final String conversationId;

  static UserQuestionNotificationTarget? fromPayload(String? payload) {
    if (payload == null || payload.length > 1024) return null;
    try {
      final value = jsonDecode(payload);
      if (value is! Map<String, dynamic>) return null;
      final groupId = value['groupId'];
      final conversationId = value['conversationId'];
      if (groupId is! String ||
          conversationId is! String ||
          !_isSafeIdentifier(groupId) ||
          !_isSafeIdentifier(conversationId)) {
        return null;
      }
      return UserQuestionNotificationTarget(
        groupId: groupId,
        conversationId: conversationId,
      );
    } on FormatException {
      return null;
    }
  }

  static bool _isSafeIdentifier(String value) =>
      value.isNotEmpty &&
      value.length <= 128 &&
      value.codeUnits.every(
        (unit) =>
            (unit >= 48 && unit <= 57) ||
            (unit >= 65 && unit <= 90) ||
            (unit >= 97 && unit <= 122) ||
            unit == 45 ||
            unit == 95,
      );
}

class UserQuestionNotifications {
  UserQuestionNotifications({FlutterLocalNotificationsPlugin? plugin})
    : _plugin = plugin ?? FlutterLocalNotificationsPlugin();

  final FlutterLocalNotificationsPlugin _plugin;
  bool _initialized = false;

  Future<UserQuestionNotificationTarget?> initialize({
    required void Function(UserQuestionNotificationTarget target) onSelected,
  }) async {
    if (!Platform.isWindows ||
        defaultTargetPlatform != TargetPlatform.windows ||
        _initialized) {
      return null;
    }
    await _plugin.initialize(
      settings: const InitializationSettings(
        windows: WindowsInitializationSettings(
          appName: 'OpenChat',
          appUserModelId: 'OpenChat.Desktop',
          guid: '7db420de-9ea4-4e36-91f5-8d2224017381',
        ),
      ),
      onDidReceiveNotificationResponse: (response) {
        final target = UserQuestionNotificationTarget.fromPayload(
          response.payload,
        );
        if (target != null) onSelected(target);
      },
    );
    _initialized = true;
    final launchDetails = await _plugin.getNotificationAppLaunchDetails();
    if (launchDetails?.didNotificationLaunchApp != true) return null;
    return UserQuestionNotificationTarget.fromPayload(
      launchDetails?.notificationResponse?.payload,
    );
  }

  Future<void> show({
    required String groupId,
    required String conversationId,
    required String title,
    required String body,
  }) async {
    if (!Platform.isWindows ||
        defaultTargetPlatform != TargetPlatform.windows ||
        !_initialized) {
      return;
    }
    final payload = jsonEncode(<String, Object?>{
      'groupId': groupId,
      'conversationId': conversationId,
    });
    await _plugin.show(
      id: groupId.hashCode & 0x7fffffff,
      title: title,
      body: body,
      notificationDetails: const NotificationDetails(
        windows: WindowsNotificationDetails(),
      ),
      payload: payload,
    );
  }
}
