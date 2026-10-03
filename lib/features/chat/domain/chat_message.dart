import 'package:openchat/features/chat/domain/chat_attachment.dart';

enum ChatMessageRole { user, assistant }

enum ChatMessageStatus { streaming, completed, failed, stopped }

enum ChatToolActivityStatus {
  awaitingApproval,
  waitingForUser,
  running,
  completed,
  failed,
  denied,
  cancelled,
}

class ChatToolActivity {
  const ChatToolActivity({
    required this.callId,
    required this.name,
    required this.arguments,
    required this.status,
    this.roundId,
    this.assistantTextBeforeByteOffset,
    this.output,
    this.targetPath,
  });

  final String callId;
  final String name;
  final Object? arguments;
  final String? roundId;
  final int? assistantTextBeforeByteOffset;
  final Object? output;
  final String? targetPath;
  final ChatToolActivityStatus status;

  Map<String, Object?> toJson() => <String, Object?>{
    'callId': callId,
    'name': name,
    'arguments': arguments,
    if (roundId != null) 'roundId': roundId,
    if (assistantTextBeforeByteOffset != null)
      'assistantTextBeforeByteOffset': assistantTextBeforeByteOffset,
    if (output != null) 'output': output,
    if (targetPath != null) 'targetPath': targetPath,
    'status': status.name,
  };

  static ChatToolActivity fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A tool activity was invalid.');
    }
    final callId = value['callId'];
    final name = value['name'];
    final statusValue = value['status'];
    final roundId = value['roundId'];
    final assistantTextBeforeByteOffset =
        value['assistantTextBeforeByteOffset'];
    final targetPath = value['targetPath'];
    if (callId is! String ||
        callId.isEmpty ||
        name is! String ||
        name.isEmpty ||
        !value.containsKey('arguments') ||
        statusValue is! String ||
        (roundId != null && (roundId is! String || roundId.isEmpty)) ||
        (assistantTextBeforeByteOffset != null &&
            (assistantTextBeforeByteOffset is! int ||
                assistantTextBeforeByteOffset < 0)) ||
        (targetPath != null && targetPath is! String)) {
      throw const FormatException('A tool activity was invalid.');
    }
    final status = switch (statusValue) {
      'awaitingApproval' => ChatToolActivityStatus.awaitingApproval,
      'waitingForUser' => ChatToolActivityStatus.waitingForUser,
      'running' => ChatToolActivityStatus.running,
      'completed' => ChatToolActivityStatus.completed,
      'failed' => ChatToolActivityStatus.failed,
      'denied' => ChatToolActivityStatus.denied,
      'cancelled' => ChatToolActivityStatus.cancelled,
      _ => throw const FormatException('A tool activity status was invalid.'),
    };
    final hasOutput = value.containsKey('output');
    final requiresOutput =
        status == ChatToolActivityStatus.completed ||
        status == ChatToolActivityStatus.failed ||
        status == ChatToolActivityStatus.denied ||
        status == ChatToolActivityStatus.cancelled;
    if (hasOutput != requiresOutput) {
      throw const FormatException('A tool activity result was invalid.');
    }
    return ChatToolActivity(
      callId: callId,
      name: name,
      arguments: value['arguments'],
      roundId: roundId is String ? roundId : null,
      assistantTextBeforeByteOffset: assistantTextBeforeByteOffset is int
          ? assistantTextBeforeByteOffset
          : null,
      output: value['output'],
      targetPath: targetPath is String ? targetPath : null,
      status: status,
    );
  }

  static List<ChatToolActivity> listFromJson(Object? value) {
    if (value is! List<Object?>) {
      throw const FormatException('The tool activity list was invalid.');
    }
    return value.map(ChatToolActivity.fromJson).toList(growable: false);
  }
}

class ChatReasoningSummary {
  const ChatReasoningSummary({
    required this.id,
    required this.content,
    required this.isComplete,
    this.elapsed,
  });

  final String id;
  final String content;
  final Duration? elapsed;
  final bool isComplete;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'content': content,
    'elapsedMicroseconds': elapsed?.inMicroseconds,
    'isComplete': isComplete,
  };

  static ChatReasoningSummary fromJson(Object? value) {
    if (value is! Map<String, Object?>) {
      throw const FormatException('A reasoning summary was invalid.');
    }
    final id = value['id'];
    final content = value['content'];
    final elapsedMicroseconds = value['elapsedMicroseconds'];
    final isComplete = value['isComplete'];
    if (id is! String ||
        id.isEmpty ||
        content is! String ||
        (elapsedMicroseconds != null && elapsedMicroseconds is! int) ||
        isComplete is! bool) {
      throw const FormatException('A reasoning summary was invalid.');
    }
    if (elapsedMicroseconds case final int elapsed when elapsed < 0) {
      throw const FormatException('A reasoning summary duration was invalid.');
    }
    return ChatReasoningSummary(
      id: id,
      content: content,
      elapsed: elapsedMicroseconds is int
          ? Duration(microseconds: elapsedMicroseconds)
          : null,
      isComplete: isComplete,
    );
  }

  static List<ChatReasoningSummary> listFromJson(Object? value) {
    if (value is! List<Object?>) {
      throw const FormatException('The reasoning summary list was invalid.');
    }
    return value.map(ChatReasoningSummary.fromJson).toList(growable: false);
  }
}

class ChatMessage {
  const ChatMessage({
    required this.id,
    required this.role,
    required this.content,
    this.attachments = const <ChatAttachment>[],
    this.createdAt,
    this.outputTokens,
    this.tokensPerSecond,
    this.elapsed,
    this.reasoningSummaries = const <ChatReasoningSummary>[],
    this.toolActivities = const <ChatToolActivity>[],
    this.status = ChatMessageStatus.completed,
    this.failureCode,
  });

  final String id;
  final ChatMessageRole role;
  final String content;
  final List<ChatAttachment> attachments;
  final DateTime? createdAt;
  final int? outputTokens;
  final double? tokensPerSecond;
  final Duration? elapsed;
  final List<ChatReasoningSummary> reasoningSummaries;
  final List<ChatToolActivity> toolActivities;
  final ChatMessageStatus status;
  final String? failureCode;
}
