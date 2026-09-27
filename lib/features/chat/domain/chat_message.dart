enum ChatMessageRole { user, assistant }

enum ChatMessageStatus { streaming, completed, failed, stopped }

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
    return value
        .map(ChatReasoningSummary.fromJson)
        .toList(growable: false);
  }
}

class ChatMessage {
  const ChatMessage({
    required this.id,
    required this.role,
    required this.content,
    this.createdAt,
    this.outputTokens,
    this.tokensPerSecond,
    this.elapsed,
    this.reasoningSummaries = const <ChatReasoningSummary>[],
    this.status = ChatMessageStatus.completed,
  });

  final String id;
  final ChatMessageRole role;
  final String content;
  final DateTime? createdAt;
  final int? outputTokens;
  final double? tokensPerSecond;
  final Duration? elapsed;
  final List<ChatReasoningSummary> reasoningSummaries;
  final ChatMessageStatus status;
}
