import 'dart:convert';
import 'dart:io';
import 'dart:ui' show AppLifecycleState;

import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';

enum ConversationNotificationKind {
  userQuestion,
  assistantResponse,
  assistantResponseReply,
}

class ConversationNotificationTarget {
  const ConversationNotificationTarget({
    required this.kind,
    required this.conversationId,
    this.groupId,
    this.assistantMessageId,
    this.replyText,
    this.replyInputTooLong = false,
  });

  final ConversationNotificationKind kind;
  final String conversationId;
  final String? groupId;
  final String? assistantMessageId;
  final String? replyText;
  final bool replyInputTooLong;

  static ConversationNotificationTarget? fromPayload(String? payload) {
    if (payload == null || payload.length > 1024) return null;
    try {
      final value = jsonDecode(payload);
      if (value is! Map<String, dynamic>) return null;
      final groupId = value['groupId'];
      final conversationId = value['conversationId'];
      if (conversationId is! String || !_isSafeIdentifier(conversationId)) {
        return null;
      }
      if (value['type'] == 'assistant_response' && groupId == null) {
        return ConversationNotificationTarget(
          kind: ConversationNotificationKind.assistantResponse,
          conversationId: conversationId,
        );
      }
      if (value['type'] == 'assistant_response_reply' && groupId == null) {
        final assistantMessageId = value['assistantMessageId'];
        if (assistantMessageId is! String ||
            !_isSafeIdentifier(assistantMessageId)) {
          return null;
        }
        return ConversationNotificationTarget(
          kind: ConversationNotificationKind.assistantResponseReply,
          conversationId: conversationId,
          assistantMessageId: assistantMessageId,
        );
      }
      if (value['type'] == null &&
          groupId is String &&
          _isSafeIdentifier(groupId)) {
        return ConversationNotificationTarget(
          kind: ConversationNotificationKind.userQuestion,
          groupId: groupId,
          conversationId: conversationId,
        );
      }
      return null;
    } on FormatException {
      return null;
    }
  }

  static ConversationNotificationTarget? fromNotificationResponse(
    NotificationResponse? response,
  ) {
    final target = fromPayload(response?.payload);
    if (target == null ||
        target.kind != ConversationNotificationKind.assistantResponseReply) {
      return target;
    }
    final assistantMessageId = target.assistantMessageId;
    if (assistantMessageId == null) return null;
    final rawReply =
        response?.data[UserQuestionNotifications.assistantReplyInputId];
    if (rawReply is! String) {
      return ConversationNotificationTarget(
        kind: ConversationNotificationKind.assistantResponseReply,
        conversationId: target.conversationId,
        assistantMessageId: assistantMessageId,
      );
    }
    if (rawReply.length >
            UserQuestionNotifications.maximumAssistantReplyRunes * 2 ||
        rawReply.runes.length >
            UserQuestionNotifications.maximumAssistantReplyRunes) {
      return ConversationNotificationTarget(
        kind: ConversationNotificationKind.assistantResponseReply,
        conversationId: target.conversationId,
        assistantMessageId: assistantMessageId,
        replyInputTooLong: true,
      );
    }
    return ConversationNotificationTarget(
      kind: ConversationNotificationKind.assistantResponseReply,
      conversationId: target.conversationId,
      assistantMessageId: assistantMessageId,
      replyText: rawReply.trim(),
    );
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
  static const assistantReplyInputId = 'assistant_reply';
  static const maximumAssistantReplyRunes = 4096;

  UserQuestionNotifications({FlutterLocalNotificationsPlugin? plugin})
    : _plugin = plugin ?? FlutterLocalNotificationsPlugin();

  final FlutterLocalNotificationsPlugin _plugin;
  bool _initialized = false;

  static bool isApplicationBackgrounded(AppLifecycleState? lifecycleState) =>
      lifecycleState != AppLifecycleState.resumed;

  Future<ConversationNotificationTarget?> initialize({
    required void Function(ConversationNotificationTarget target) onSelected,
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
        final target = ConversationNotificationTarget.fromNotificationResponse(
          response,
        );
        if (target != null) onSelected(target);
      },
    );
    _initialized = true;
    final launchDetails = await _plugin.getNotificationAppLaunchDetails();
    if (launchDetails?.didNotificationLaunchApp != true) return null;
    return ConversationNotificationTarget.fromNotificationResponse(
      launchDetails?.notificationResponse,
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

  Future<void> showAssistantResponse({
    required String conversationId,
    required String assistantMessageId,
    required String providerId,
    required String title,
    required String content,
    required String replyLabel,
    required String sendLabel,
  }) async {
    if (!Platform.isWindows ||
        defaultTargetPlatform != TargetPlatform.windows ||
        !_initialized) {
      return;
    }
    final iconAsset = providerIconAssetFor(providerId);
    final payload = jsonEncode(<String, Object?>{
      'type': 'assistant_response',
      'conversationId': conversationId,
    });
    final replyPayload = jsonEncode(<String, Object?>{
      'type': 'assistant_response_reply',
      'conversationId': conversationId,
      'assistantMessageId': assistantMessageId,
    });
    final details = NotificationDetails(
      windows: WindowsNotificationDetails(
        inputs: <WindowsInput>[
          WindowsTextInput(
            id: assistantReplyInputId,
            title: replyLabel,
            placeHolderContent: replyLabel,
          ),
        ],
        actions: <WindowsAction>[
          WindowsAction(
            content: sendLabel,
            arguments: replyPayload,
            inputId: assistantReplyInputId,
          ),
        ],
        images: <WindowsImage>[
          WindowsImage(
            WindowsImage.getAssetUri(iconAsset),
            altText: '$providerId provider icon',
            placement: WindowsImagePlacement.appLogoOverride,
            crop: WindowsImageCrop.circle,
          ),
        ],
      ),
    );
    await _plugin.show(
      id: 'assistant_response_$conversationId'.hashCode & 0x7fffffff,
      title: title,
      body: assistantResponsePreview(content),
      notificationDetails: details,
      payload: payload,
    );
  }

  static String assistantResponsePreview(String content) {
    final normalized = content.replaceAll(RegExp(r'\s+'), ' ').trim();
    final characters = normalized.runes;
    if (characters.length <= 160) return normalized;
    return '${String.fromCharCodes(characters.take(159))}…';
  }

  @visibleForTesting
  static String providerIconAssetFor(String providerId) => switch (providerId) {
    'chatgpt' => 'assets/icons/notification_chatgpt.svg',
    'opencode' => 'assets/icons/notification_opencode.svg',
    'gemini' => 'assets/icons/notification_sparkles.svg',
    'groq' => 'assets/icons/notification_zap.svg',
    'cerebras' => 'assets/icons/notification_microchip.svg',
    'openrouter' => 'assets/icons/notification_openrouter.svg',
    'mistral' => 'assets/icons/mistral.png',
    'llama_cpp' => 'assets/icons/notification_llama_cpp.svg',
    'vllm' => 'assets/icons/notification_vllm.svg',
    'exllama' => 'assets/icons/engines/exllama-v3.png',
    _ => throw ArgumentError.value(providerId, 'providerId'),
  };
}
